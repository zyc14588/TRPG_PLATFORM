// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type PlatformTaskOptions struct {
	Repository      *PlatformAuthRepository
	Lease, Lifetime time.Duration
	Fault           func(context.Context, string) error
}
type PlatformTaskStorage struct{ data **platformTaskData }
type platformTaskData struct {
	repo            *PlatformAuthRepository
	lease, lifetime time.Duration
	fault           func(context.Context, string) error
}

func (PlatformTaskStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private task storage>")
}
func (PlatformTaskStorage) MarshalJSON() ([]byte, error) { return nil, task.ErrDenied }
func (s *PlatformTaskStorage) state() *platformTaskData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func NewPlatformTaskStorage(o PlatformTaskOptions) (*PlatformTaskStorage, error) {
	if o.Repository == nil || o.Repository.state() == nil || o.Lease < 50*time.Millisecond || o.Lease > 30*time.Second || o.Lifetime < 100*time.Millisecond || o.Lifetime > 10*time.Minute || o.Lifetime <= o.Lease {
		return nil, task.ErrInvalid
	}
	d := &platformTaskData{repo: o.Repository, lease: o.Lease, lifetime: o.Lifetime, fault: o.Fault}
	return &PlatformTaskStorage{data: &d}, nil
}
func taskSQL(e error) error {
	if e == nil {
		return nil
	}
	if errors.Is(e, sql.ErrNoRows) {
		return task.ErrNotFound
	}
	if errors.Is(e, context.Canceled) || errors.Is(e, context.DeadlineExceeded) {
		return task.SafeError(e)
	}
	return task.ErrUnavailable
}
func (s *PlatformTaskStorage) inject(ctx context.Context, p string) (err error) {
	defer func() {
		if recover() != nil {
			err = task.ErrUnavailable
		}
	}()
	if ctx.Err() != nil {
		return task.SafeError(ctx.Err())
	}
	if s.state().fault != nil {
		return task.SafeError(s.state().fault(ctx, p))
	}
	return nil
}
func (s *PlatformTaskStorage) begin(ctx context.Context, w task.Worker) (*sql.Tx, time.Time, error) {
	if s.state() == nil || ctx == nil || ctx.Err() != nil {
		return nil, time.Time{}, task.ErrDenied
	}
	if _, e := w.Workspaces(ctx); e != nil {
		return nil, time.Time{}, task.ErrDenied
	}
	tx, e := s.state().repo.state().core.state().db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
	if e != nil {
		return nil, time.Time{}, taskSQL(e)
	}
	for _, q := range []string{`SET LOCAL statement_timeout='2s'`, `SET LOCAL lock_timeout='2s'`} {
		if _, e = tx.ExecContext(ctx, q); e != nil {
			tx.Rollback()
			return nil, time.Time{}, taskSQL(e)
		}
	}
	var now time.Time
	if e = tx.QueryRowContext(ctx, `SELECT clock_timestamp()`).Scan(&now); e != nil {
		tx.Rollback()
		return nil, now, taskSQL(e)
	}
	return tx, now, nil
}
func (s *PlatformTaskStorage) commit(ctx context.Context, tx *sql.Tx, w task.Worker, space, point string) error {
	if w.Check(ctx, space) != nil {
		return task.ErrDenied
	}
	if e := s.inject(ctx, "task-before-"+point+"-commit"); e != nil {
		return e
	}
	if e := tx.Commit(); e != nil {
		return taskSQL(e)
	}
	return s.inject(ctx, "task-after-"+point+"-commit")
}

type taskRow struct {
	job    task.JobData
	digest string
}

// Every indexed column is compared with the bounded body. Caller-supplied
// fields and a deserialized job can never replace the current database lease.
func scanTask(row interface{ Scan(...any) error }) (taskRow, error) {
	var raw []byte
	var space, session, id, outbox, status, owner, digest string
	var attempt int
	var until, expires time.Time
	if e := row.Scan(&space, &session, &id, &outbox, &status, &attempt, &owner, &digest, &until, &expires, &raw); e != nil {
		return taskRow{}, taskSQL(e)
	}
	j, e := task.DecodeStored(raw)
	if e != nil {
		return taskRow{}, task.ErrDenied
	}
	v, e := j.StorageValue()
	if e != nil || v.Binding.Workspace != space || v.Binding.Session != session || v.TaskID != id || v.OutboxID != outbox || v.Status != status || v.Attempt != attempt || v.LeaseOwner != owner || !v.LeaseUntil.Equal(until) || !v.Expires.Equal(expires) {
		return taskRow{}, task.ErrDenied
	}
	return taskRow{job: v, digest: digest}, nil
}

const taskColumns = `workspace,session,task_id,outbox_id,status,attempt,lease_owner,lease_digest,lease_until,expires,body`

func putTask(ctx context.Context, tx *sql.Tx, v task.JobData, digest string, insert bool) error {
	j, e := task.NewJob(v)
	if e != nil {
		return e
	}
	raw, e := task.EncodeForStorage(j)
	if e != nil {
		return e
	}
	if insert {
		_, e = tx.ExecContext(ctx, `INSERT INTO platform_task.jobs(`+taskColumns+`) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT DO NOTHING`, v.Binding.Workspace, v.Binding.Session, v.TaskID, v.OutboxID, v.Status, v.Attempt, v.LeaseOwner, digest, v.LeaseUntil, v.Expires, raw)
		return taskSQL(e)
	}
	r, e := tx.ExecContext(ctx, `UPDATE platform_task.jobs SET status=$4,attempt=$5,lease_owner=$6,lease_digest=$7,lease_until=$8,expires=$9,body=$10 WHERE workspace=$1 AND session=$2 AND task_id=$3`, v.Binding.Workspace, v.Binding.Session, v.TaskID, v.Status, v.Attempt, v.LeaseOwner, digest, v.LeaseUntil, v.Expires, raw)
	if e != nil {
		return taskSQL(e)
	}
	n, e := r.RowsAffected()
	if e != nil || n != 1 {
		return task.ErrDenied
	}
	return nil
}
func lockTask(ctx context.Context, tx *sql.Tx, b data.Binding, id string) (taskRow, error) {
	return scanTask(tx.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM platform_task.jobs WHERE workspace=$1 AND session=$2 AND task_id=$3 FOR UPDATE`, b.Workspace, b.Session, id))
}
func leaseTask(ctx context.Context, tx *sql.Tx, w task.Worker, j task.Job, now time.Time, allowExpired bool) (taskRow, error) {
	v, e := j.StorageValue()
	if e != nil || w.Check(ctx, v.Binding.Workspace) != nil || !validBinding(v.Binding) || !store.ValidID(v.TaskID) || v.Token.Digest() == "" {
		return taskRow{}, task.ErrDenied
	}
	r, e := lockTask(ctx, tx, v.Binding, v.TaskID)
	if e != nil {
		return taskRow{}, e
	}
	if r.job.Binding != v.Binding || r.job.Scope != v.Scope || r.job.LeaseOwner != w.ID() || r.digest != v.Token.Digest() || (r.job.Status != task.Running && r.job.Status != task.Delivering) {
		return taskRow{}, task.ErrDenied
	}
	if !allowExpired && (!r.job.Expires.After(now) || !r.job.LeaseUntil.After(now)) {
		return taskRow{}, task.ErrExpired
	}
	r.job.Token = v.Token
	return r, nil
}

func (s *PlatformTaskStorage) Claim(ctx context.Context, w task.Worker) (task.Job, error) {
	tx, now, e := s.begin(ctx, w)
	if e != nil {
		return task.Job{}, e
	}
	defer tx.Rollback()
	spaces, e := w.Workspaces(ctx)
	if e != nil {
		return task.Job{}, e
	}
	budget := 8
	for _, space := range spaces {
		// This operational lock only bounds queue growth. No game/state row is
		// locked, and it is released before any external operation starts.
		if _, e = tx.ExecContext(ctx, `SELECT pg_advisory_xact_lock(hashtextextended($1,0))`, "platform-task:"+space); e != nil {
			return task.Job{}, taskSQL(e)
		}
		if budget > 0 {
			n, e := s.seed(ctx, tx, space, now, budget)
			if e != nil {
				return task.Job{}, e
			}
			budget -= n
		}
		for visited := 0; visited < 64; visited++ {
			r, e := scanTask(tx.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM platform_task.jobs WHERE workspace=$1 AND (status IN ('queued','ready') OR (status IN ('running','delivering') AND lease_until<=$2)) ORDER BY expires,session,task_id LIMIT 1 FOR UPDATE SKIP LOCKED`, space, now))
			if errors.Is(e, task.ErrNotFound) {
				break
			}
			if e != nil {
				return task.Job{}, e
			}
			v := r.job
			if !v.Expires.After(now) {
				v.Status = task.Expired
				v.Token = task.Token{}
				if e = putTask(ctx, tx, v, "", false); e != nil {
					return task.Job{}, e
				}
				continue
			}
			if v.ResultWorker == "" && v.Attempt >= task.MaxAttempts {
				v.Status = task.Failed
				if e = putTask(ctx, tx, v, "", false); e != nil {
					return task.Job{}, e
				}
				continue
			}
			if v.ResultWorker != "" && v.DeliveryAttempt >= task.MaxAttempts {
				if _, e = taskReceipt(ctx, tx, v); e != nil {
					if !errors.Is(e, task.ErrNotFound) && !errors.Is(e, task.ErrDenied) {
						return task.Job{}, e
					}
					v.Status = task.Failed
					if e = putTask(ctx, tx, v, "", false); e != nil {
						return task.Job{}, e
					}
					continue
				}
			}
			if e = taskLive(ctx, tx, v); e != nil {
				if errors.Is(e, task.ErrStale) {
					v.Status = task.Stale
				} else if errors.Is(e, task.ErrDenied) {
					v.Status = task.Cancelled
				} else {
					return task.Job{}, e
				}
				if e = putTask(ctx, tx, v, "", false); e != nil {
					return task.Job{}, e
				}
				continue
			}
			v.Token, e = task.NewToken()
			if e != nil {
				return task.Job{}, e
			}
			v.LeaseOwner = w.ID()
			v.LeaseUntil = now.Add(s.state().lease)
			if v.Expires.Before(v.LeaseUntil) {
				v.LeaseUntil = v.Expires
			}
			if v.ResultWorker == "" {
				v.Status = task.Running
				v.Attempt++
			} else {
				v.Status = task.Delivering
				if v.DeliveryAttempt < task.MaxAttempts {
					v.DeliveryAttempt++
				}
			}
			if e = putTask(ctx, tx, v, v.Token.Digest(), false); e != nil {
				return task.Job{}, e
			}
			if e = s.commit(ctx, tx, w, space, "claim"); e != nil {
				return task.Job{}, e
			}
			return task.NewJob(v)
		}
	}
	if e = tx.Commit(); e != nil {
		return task.Job{}, taskSQL(e)
	}
	return task.Job{}, task.ErrNotFound
}

func (s *PlatformTaskStorage) SaveResult(ctx context.Context, w task.Worker, j task.Job, result task.Value, inputs task.Inputs) (task.Job, error) {
	if _, e := result.StorageValue(); e != nil {
		return task.Job{}, task.ErrInvalid
	}
	if _, _, e := inputs.StorageValue(); e != nil {
		return task.Job{}, task.ErrInvalid
	}
	tx, now, e := s.begin(ctx, w)
	if e != nil {
		return task.Job{}, e
	}
	defer tx.Rollback()
	r, e := leaseTask(ctx, tx, w, j, now, false)
	if e != nil {
		return task.Job{}, e
	}
	v := r.job
	if v.Status != task.Running || v.ResultWorker != "" {
		return task.Job{}, task.ErrDenied
	}
	if e = taskLive(ctx, tx, v); e != nil {
		return task.Job{}, e
	}
	v.Result = result
	v.Inputs = inputs
	v.ResultWorker = w.ID()
	v.Status = task.Delivering
	v.DeliveryAttempt = 1
	if len(v.Continuations) == 0 {
		v.Status = task.Done
	}
	if e = putTask(ctx, tx, v, r.digest, false); e != nil {
		return task.Job{}, e
	}
	if e = s.commit(ctx, tx, w, v.Binding.Workspace, "result"); e != nil {
		return task.Job{}, e
	}
	return task.NewJob(v)
}
func (s *PlatformTaskStorage) Current(ctx context.Context, w task.Worker, j task.Job) (task.Job, error) {
	tx, now, e := s.begin(ctx, w)
	if e != nil {
		return task.Job{}, e
	}
	defer tx.Rollback()
	r, e := leaseTask(ctx, tx, w, j, now, false)
	if e != nil {
		return task.Job{}, e
	}
	if r.job.Status != task.Delivering {
		return task.Job{}, task.ErrDenied
	}
	if e = taskLive(ctx, tx, r.job); e != nil {
		return task.Job{}, e
	}
	if w.Check(ctx, r.job.Binding.Workspace) != nil {
		return task.Job{}, task.ErrDenied
	}
	if e = tx.Commit(); e != nil {
		return task.Job{}, taskSQL(e)
	}
	return task.NewJob(r.job)
}
func (s *PlatformTaskStorage) Complete(ctx context.Context, w task.Worker, j task.Job, receipt data.Receipt) error {
	tx, now, e := s.begin(ctx, w)
	if e != nil {
		return e
	}
	defer tx.Rollback()
	r, e := leaseTask(ctx, tx, w, j, now, true)
	if e != nil {
		return e
	}
	v := r.job
	if v.Status != task.Delivering {
		return task.ErrDenied
	}
	saved, e := taskReceipt(ctx, tx, v)
	if e != nil || !sameTaskReceipt(saved, receipt) {
		return task.ErrDenied
	}
	v.Next++
	v.DeliveryAttempt = 0
	v.Status = task.Ready
	if v.Next == len(v.Continuations) {
		v.Status = task.Done
	}
	v.LeaseOwner = ""
	v.LeaseUntil = time.Time{}
	v.Token = task.Token{}
	if e = putTask(ctx, tx, v, "", false); e != nil {
		return e
	}
	return s.commit(ctx, tx, w, v.Binding.Workspace, "ack")
}
func (s *PlatformTaskStorage) Retry(ctx context.Context, w task.Worker, j task.Job) error {
	tx, now, e := s.begin(ctx, w)
	if e != nil {
		return e
	}
	defer tx.Rollback()
	r, e := leaseTask(ctx, tx, w, j, now, true)
	if e != nil {
		return e
	}
	v := r.job
	if !v.Expires.After(now) {
		v.Status = task.Expired
	} else if v.ResultWorker != "" {
		v.Status = task.Ready
	} else if v.Attempt >= task.MaxAttempts {
		v.Status = task.Failed
	} else {
		v.Status = task.Queued
	}
	v.LeaseOwner = ""
	v.LeaseUntil = time.Time{}
	v.Token = task.Token{}
	if e = putTask(ctx, tx, v, "", false); e != nil {
		return e
	}
	return s.commit(ctx, tx, w, v.Binding.Workspace, "retry")
}
func (s *PlatformTaskStorage) Cancel(ctx context.Context, w task.Worker, b data.Binding, id string) error {
	if !validBinding(b) || !store.ValidID(id) || w.Check(ctx, b.Workspace) != nil {
		return task.ErrDenied
	}
	tx, _, e := s.begin(ctx, w)
	if e != nil {
		return e
	}
	defer tx.Rollback()
	r, e := lockTask(ctx, tx, b, id)
	if e != nil {
		return e
	}
	v := r.job
	if v.Binding != b {
		return task.ErrDenied
	}
	if v.Status == task.Delivering {
		return task.ErrBusy
	}
	switch v.Status {
	case task.Done, task.Failed, task.Cancelled, task.Expired, task.Stale:
		return nil
	}
	v.Status = task.Cancelled
	v.LeaseOwner = ""
	v.LeaseUntil = time.Time{}
	if e = putTask(ctx, tx, v, "", false); e != nil {
		return e
	}
	return s.commit(ctx, tx, w, b.Workspace, "cancel")
}
func (s *PlatformTaskStorage) Status(ctx context.Context, w task.Worker, b data.Binding, id string) (string, error) {
	if !validBinding(b) || !store.ValidID(id) || w.Check(ctx, b.Workspace) != nil {
		return "", task.ErrDenied
	}
	tx, _, e := s.begin(ctx, w)
	if e != nil {
		return "", e
	}
	defer tx.Rollback()
	r, e := lockTask(ctx, tx, b, id)
	if e != nil {
		return "", e
	}
	if r.job.Binding != b {
		return "", task.ErrDenied
	}
	if e = tx.Commit(); e != nil {
		return "", taskSQL(e)
	}
	return r.job.Status, nil
}

var _ task.Storage = (*PlatformTaskStorage)(nil)
