// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"regexp"
	"sync"
	"time"

	"github.com/jackc/pgx/v5/stdlib"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

type PlatformPlayerStorage struct{ data **platformPlayerData }
type platformPlayerData struct{ repo *PlatformAuthRepository }
type platformPlayerTransaction struct{ data **platformPlayerTxData }
type platformPlayerTxData struct{ tx *sql.Tx }

func (PlatformPlayerStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player storage>")
}
func (PlatformPlayerStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (platformPlayerTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player transaction>")
}
func (platformPlayerTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *PlatformPlayerStorage) state() *platformPlayerData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func (t *platformPlayerTransaction) state() *platformPlayerTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func playerSQL(tx *sql.Tx) *platformPlayerTransaction {
	d := &platformPlayerTxData{tx}
	return &platformPlayerTransaction{data: &d}
}
func NewPlatformPlayerStorage(repo *PlatformAuthRepository) (*PlatformPlayerStorage, error) {
	if repo == nil || repo.state() == nil {
		return nil, auth.ErrInvalid
	}
	d := &platformPlayerData{repo}
	return &PlatformPlayerStorage{data: &d}, nil
}
func (s *PlatformPlayerStorage) Bootstrap(ctx context.Context) error {
	if s.state() == nil || ctx == nil || ctx.Err() != nil {
		return auth.ErrUnavailable
	}
	tx, e := s.state().repo.state().core.state().db.BeginTx(ctx, nil)
	if e != nil {
		return authStorageError(e)
	}
	defer tx.Rollback()
	for _, q := range []string{
		`SET LOCAL statement_timeout='3s'`, `SET LOCAL lock_timeout='3s'`,
		`CREATE SCHEMA IF NOT EXISTS platform_player`,
		`CREATE TABLE IF NOT EXISTS platform_player.control(
		 workspace_id text NOT NULL,room_id text NOT NULL,game_id text NOT NULL,session_id text NOT NULL,
		 graph_hash text NOT NULL CHECK(graph_hash ~ '^sha256:[a-f0-9]{64}$'),
		 revision bigint NOT NULL CHECK(revision>0 AND revision<9223372036854775807),paused boolean NOT NULL,
		 body bytea NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 16384),
		 PRIMARY KEY(workspace_id,session_id),
		 FOREIGN KEY(workspace_id,room_id,game_id) REFERENCES platform_launch.sessions(workspace_id,room_id,game_id),
		 FOREIGN KEY(workspace_id,session_id) REFERENCES host_command.sessions(workspace,session))`,
		`CREATE TABLE IF NOT EXISTS platform_player.leases(
		 workspace_id text NOT NULL,session_id text NOT NULL,id text NOT NULL CHECK(id ~ '^connection-[a-f0-9]{32}$'),
		 cookie_hash text NOT NULL REFERENCES platform_auth.sessions(token_hash),participant_id text NOT NULL,seat_id text NOT NULL CHECK(seat_id ~ '^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$'),
		 expires_at timestamptz NOT NULL CHECK(isfinite(expires_at)),closed boolean NOT NULL DEFAULT false,
		 PRIMARY KEY(workspace_id,session_id,id),
		 FOREIGN KEY(workspace_id,session_id) REFERENCES platform_player.control(workspace_id,session_id))`,
		`CREATE INDEX IF NOT EXISTS platform_player_live_leases ON platform_player.leases(workspace_id,session_id,participant_id,expires_at) WHERE NOT closed`,
	} {
		if _, e = tx.ExecContext(ctx, q); e != nil {
			return authStorageError(e)
		}
	}
	if e = tx.Commit(); e != nil {
		return auth.ErrOutcomeUnknown
	}
	return nil
}
func (s *PlatformPlayerStorage) Bind(tx core.Transaction) (player.Transaction, error) {
	if s.state() == nil {
		return nil, auth.ErrUnavailable
	}
	c, ok := auth.RoomStorageCore(tx).(*platformCoreTransaction)
	if !ok || c == nil || c.tx == nil {
		return nil, auth.ErrDenied
	}
	return playerSQL(c.tx), nil
}
func (s *PlatformPlayerStorage) Transact(ctx context.Context, f func(player.Transaction) error) error {
	if s.state() == nil || ctx == nil || f == nil {
		return auth.ErrUnavailable
	}
	return auth.SafeError(s.state().repo.state().core.Transact(ctx, func(tx core.Transaction) error {
		t, e := s.Bind(tx)
		if e == nil {
			e = f(t)
		}
		switch auth.SafeError(e) {
		case nil:
			return nil
		case auth.ErrDenied:
			return core.ErrDenied
		case auth.ErrConflict:
			return core.ErrConflict
		case auth.ErrOutcomeUnknown:
			return core.ErrOutcomeUnknown
		default:
			return core.ErrUnavailable
		}
	}))
}
func (t *platformPlayerTransaction) Now(ctx context.Context) (time.Time, error) {
	var now time.Time
	e := t.state().tx.QueryRowContext(ctx, `SELECT clock_timestamp()`).Scan(&now)
	return now, authStorageError(e)
}
func (t *platformPlayerTransaction) Barrier(ctx context.Context, b data.Binding) error {
	if !validBinding(b) {
		return auth.ErrDenied
	}
	var graph string
	e := t.state().tx.QueryRowContext(ctx, `SELECT graph_hash FROM host_command.sessions WHERE workspace=$1 AND session=$2 FOR UPDATE`, b.Workspace, b.Session).Scan(&graph)
	if e != nil {
		return authStorageError(e)
	}
	if graph != b.GraphHash {
		return auth.ErrDenied
	}
	return nil
}
func (t *platformPlayerTransaction) State(ctx context.Context, b data.Binding) (player.State, error) {
	var raw []byte
	var revision uint64
	var paused bool
	var s core.Scope
	var graph string
	e := t.state().tx.QueryRowContext(ctx, `SELECT workspace_id,room_id,game_id,graph_hash,revision,paused,body FROM platform_player.control WHERE workspace_id=$1 AND session_id=$2 FOR UPDATE`, b.Workspace, b.Session).Scan(&s.WorkspaceID, &s.RoomID, &s.GameID, &graph, &revision, &paused, &raw)
	if e != nil {
		return player.State{}, authStorageError(e)
	}
	var v player.StateData
	if checkpoint.StrictDecode(raw, &v, 16384) != nil || !player.ValidState(v) || v.Scope != s || v.Binding != b || graph != b.GraphHash || v.Revision != revision || v.Paused != paused {
		return player.State{}, auth.ErrDenied
	}
	var config string
	var prep uint64
	e = t.state().tx.QueryRowContext(ctx, `SELECT configuration_hash,revision FROM platform_launch.sessions WHERE workspace_id=$1 AND session_id=$2 AND room_id=$3 AND game_id=$4 AND graph_hash=$5`, b.Workspace, b.Session, s.RoomID, s.GameID, b.GraphHash).Scan(&config, &prep)
	if e != nil || config != v.ConfigurationHash || prep != v.PreparationRevision {
		return player.State{}, auth.ErrDenied
	}
	return auth.RoomSecret(v), nil
}
func (t *platformPlayerTransaction) PutState(ctx context.Context, s player.State) error {
	v := s.StorageValue()
	if !player.ValidState(v) {
		return auth.ErrInvalid
	}
	raw, e := json.Marshal(v)
	if e != nil || len(raw) > 16384 {
		return auth.ErrInvalid
	}
	r, e := t.state().tx.ExecContext(ctx, `INSERT INTO platform_player.control(workspace_id,room_id,game_id,session_id,graph_hash,revision,paused,body) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(workspace_id,session_id) DO UPDATE SET revision=EXCLUDED.revision,paused=EXCLUDED.paused,body=EXCLUDED.body WHERE platform_player.control.room_id=EXCLUDED.room_id AND platform_player.control.game_id=EXCLUDED.game_id AND platform_player.control.graph_hash=EXCLUDED.graph_hash`, v.Scope.WorkspaceID, v.Scope.RoomID, v.Scope.GameID, v.Binding.Session, v.Binding.GraphHash, v.Revision, v.Paused, raw)
	if e != nil {
		return authStorageError(e)
	}
	n, e := r.RowsAffected()
	if e != nil || n != 1 {
		return auth.ErrDenied
	}
	return nil
}
func (t *platformPlayerTransaction) Leases(ctx context.Context, b data.Binding) ([]player.Lease, error) {
	// Expired/disabled/revoked identity rows make a lease unusable too. Its
	// original expiry/closed columns remain intact for exact owned replay checks.
	rows, e := t.state().tx.QueryContext(ctx, `SELECT l.id,l.cookie_hash,l.participant_id,l.seat_id,l.expires_at,
	 l.closed OR NOT p.active OR a.retired OR a.revoked OR a.expires_at<=clock_timestamp() OR
	 CASE WHEN a.kind='account' THEN coalesce(ac.disabled,true) OR p.account_id IS DISTINCT FROM a.account_id
	 WHEN a.kind='guest' THEN coalesce(g.disabled,true) OR g.expires_at<=clock_timestamp() OR g.claimed_account_id IS NOT NULL OR p.guest_id IS DISTINCT FROM a.guest_id OR (a.workspace_id,a.room_id,a.game_id) IS DISTINCT FROM (c.workspace_id,c.room_id,c.game_id) ELSE true END,
	 c.room_id,c.game_id,c.graph_hash
	 FROM platform_player.leases l JOIN platform_player.control c ON c.workspace_id=l.workspace_id AND c.session_id=l.session_id
	 JOIN platform_auth.sessions a ON a.token_hash=l.cookie_hash
	 JOIN platform_room.participants p ON p.workspace_id=c.workspace_id AND p.room_id=c.room_id AND p.game_id=c.game_id AND p.id=l.participant_id
	 LEFT JOIN platform_core.accounts ac ON ac.id=a.account_id
	 LEFT JOIN platform_core.guests g ON g.workspace_id=c.workspace_id AND g.room_id=c.room_id AND g.game_id=c.game_id AND g.id=a.guest_id
	 WHERE l.workspace_id=$1 AND l.session_id=$2 ORDER BY l.id LIMIT 65`, b.Workspace, b.Session)
	if e != nil {
		return nil, authStorageError(e)
	}
	defer rows.Close()
	out := []player.Lease{}
	for rows.Next() {
		v := player.LeaseData{Binding: b, Scope: core.Scope{WorkspaceID: b.Workspace}}
		var graph string
		if e = rows.Scan(&v.ID, &v.CookieHash, &v.Principal, &v.Seat, &v.ExpiresAt, &v.Closed, &v.Scope.RoomID, &v.Scope.GameID, &graph); e != nil {
			return nil, authStorageError(e)
		}
		if graph != b.GraphHash {
			return nil, auth.ErrDenied
		}
		out = append(out, auth.RoomSecret(v))
	}
	if len(out) > 64 {
		return nil, auth.ErrRateLimited
	}
	return out, authStorageError(rows.Err())
}

var playerCookieHash = regexp.MustCompile(`^[A-Za-z0-9_-]{43}$`)
var playerConnectionID = regexp.MustCompile(`^connection-[a-f0-9]{32}$`)

func (t *platformPlayerTransaction) PutLease(ctx context.Context, l player.Lease) error {
	v := l.StorageValue()
	if !playerConnectionID.MatchString(v.ID) || !playerCookieHash.MatchString(v.CookieHash) || !validBinding(v.Binding) || v.Scope.WorkspaceID != v.Binding.Workspace || !store.ValidID(v.Principal) || !store.ValidID(v.Seat) {
		return auth.ErrInvalid
	}
	// Keep bounded replay metadata; only expired/closed older rows are removed.
	_, e := t.state().tx.ExecContext(ctx, `DELETE FROM platform_player.leases WHERE workspace_id=$1 AND session_id=$2 AND id<>$3 AND (closed OR expires_at<clock_timestamp()-interval '1 minute')`, v.Binding.Workspace, v.Binding.Session, v.ID)
	if e != nil {
		return authStorageError(e)
	}
	var n int
	if e = t.state().tx.QueryRowContext(ctx, `SELECT count(*) FROM platform_player.leases WHERE workspace_id=$1 AND session_id=$2 AND id<>$3`, v.Binding.Workspace, v.Binding.Session, v.ID).Scan(&n); e != nil {
		return authStorageError(e)
	}
	if n >= 64 {
		return auth.ErrRateLimited
	}
	r, e := t.state().tx.ExecContext(ctx, `INSERT INTO platform_player.leases(workspace_id,session_id,id,cookie_hash,participant_id,seat_id,expires_at,closed) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(workspace_id,session_id,id) DO UPDATE SET expires_at=EXCLUDED.expires_at,closed=EXCLUDED.closed WHERE platform_player.leases.cookie_hash=EXCLUDED.cookie_hash AND platform_player.leases.participant_id=EXCLUDED.participant_id AND platform_player.leases.seat_id=EXCLUDED.seat_id`, v.Binding.Workspace, v.Binding.Session, v.ID, v.CookieHash, v.Principal, v.Seat, v.ExpiresAt, v.Closed)
	if e != nil {
		return authStorageError(e)
	}
	n64, e := r.RowsAffected()
	if e != nil || n64 != 1 {
		return auth.ErrDenied
	}
	return nil
}

// Closing the actual approved pgx connection removes its session locks.
// ResetSession then tells database/sql to discard that closed connection.
func discardPlayerConnection(conn *sql.Conn) {
	_ = conn.Raw(func(raw any) error {
		owned, ok := raw.(*stdlib.Conn)
		if !ok {
			if closer, ok := raw.(interface{ Close() error }); ok {
				_ = closer.Close()
			}
			return auth.ErrUnavailable
		}
		bounded, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		_ = owned.Conn().Close(bounded)
		return owned.ResetSession(bounded)
	})
}
func (s *PlatformPlayerStorage) ExecutionLock(ctx context.Context, b data.Binding, exclusive bool) (func(), error) {
	if s.state() == nil || ctx == nil || !validBinding(b) {
		return nil, auth.ErrDenied
	}
	bounded, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	conn, e := s.state().repo.state().core.state().db.Conn(bounded)
	if e != nil {
		return nil, authStorageError(e)
	}
	lock, unlock := "pg_try_advisory_lock_shared", "pg_advisory_unlock_shared"
	if exclusive {
		lock, unlock = "pg_try_advisory_lock", "pg_advisory_unlock"
	}
	key := "platform-player:" + b.Workspace + "/" + b.Session + "/" + b.GraphHash
	for {
		var got bool
		e = conn.QueryRowContext(bounded, `SELECT `+lock+`(hashtextextended($1,0))`, key).Scan(&got)
		if e != nil {
			discardPlayerConnection(conn)
			_ = conn.Close()
			return nil, auth.ErrUnavailable
		}
		if got {
			break
		}
		timer := time.NewTimer(10 * time.Millisecond)
		select {
		case <-bounded.Done():
			timer.Stop()
			_ = conn.Close()
			return nil, auth.ErrUnavailable
		case <-timer.C:
		}
	}
	var once sync.Once
	return func() {
		once.Do(func() {
			release, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			var ok bool
			if e := conn.QueryRowContext(release, `SELECT `+unlock+`(hashtextextended($1,0))`, key).Scan(&ok); e != nil || !ok {
				discardPlayerConnection(conn)
			}
			_ = conn.Close()
		})
	}, nil
}

// ControlledLaunchStorage decorates only the M2 composition's command
// repository. The accepted M1 tables, code and immutable replay bytes are intact.
type ControlledLaunchStorage struct{ data **controlledLaunchData }
type controlledLaunchData struct {
	base   *PlatformLaunchStorage
	player *PlatformPlayerStorage
}

func (ControlledLaunchStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<controlled launch storage>")
}
func (ControlledLaunchStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *ControlledLaunchStorage) state() *controlledLaunchData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func NewControlledLaunchStorage(base *PlatformLaunchStorage, p *PlatformPlayerStorage) (*ControlledLaunchStorage, error) {
	if base.state() == nil || p.state() == nil || base.state().repo != p.state().repo {
		return nil, auth.ErrInvalid
	}
	d := &controlledLaunchData{base, p}
	return &ControlledLaunchStorage{data: &d}, nil
}
func (s *ControlledLaunchStorage) Bind(tx core.Transaction) (launch.Transaction, error) {
	return s.state().base.Bind(tx)
}
func (s *ControlledLaunchStorage) RuntimeRepository() persistence.Repository {
	return s.state().base.RuntimeRepository()
}
func (s *ControlledLaunchStorage) SessionRepository() install.SessionRepository {
	d := &playerSessionRepoData{s.state().base.SessionRepository(), s.state().player}
	return &playerSessionRepository{data: &d}
}

type playerSessionRepository struct{ data **playerSessionRepoData }
type playerSessionRepoData struct {
	base   install.SessionRepository
	player *PlatformPlayerStorage
}

func (playerSessionRepository) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<controlled native repository>")
}
func (playerSessionRepository) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (r *playerSessionRepository) state() *playerSessionRepoData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}
func (r *playerSessionRepository) ProvisionGraph(c context.Context, g *store.Graph, b data.Binding, v uint64, h string, s checkpoint.Value) error {
	return r.state().base.ProvisionGraph(c, g, b, v, h, s)
}
func (r *playerSessionRepository) ReadGraphSession(c context.Context, g *store.Graph, b data.Binding) (data.Snapshot, error) {
	return r.state().base.ReadGraphSession(c, g, b)
}
func nativePlayerReady(ctx context.Context, tx *sql.Tx, h data.Header) error {
	t := playerSQL(tx)
	state, e := t.State(ctx, h.Binding)
	if e != nil {
		return data.ErrDenied
	}
	v := state.StorageValue()
	if v.Paused {
		return player.ErrPaused
	}
	leases, e := t.Leases(ctx, h.Binding)
	if e != nil {
		return data.ErrDenied
	}
	now, e := t.Now(ctx)
	if e != nil {
		return data.ErrDenied
	}
	if player.Disconnected(v, leases, now) {
		return player.ErrConnectionExpired
	}
	if h.Principal != "task-system" {
		issued := player.NativeLease(ctx).StorageValue()
		found := false
		for _, l := range leases {
			x := l.StorageValue()
			if x.ID == issued.ID && x.CookieHash == issued.CookieHash && x.Principal == h.Principal && x.Principal == issued.Principal && x.Seat == issued.Seat && x.Binding == h.Binding && x.Scope == issued.Scope && !x.Closed && now.Before(x.ExpiresAt) {
				found = true
			}
		}
		if !found {
			return player.ErrConnectionExpired
		}
	}
	return nil
}
func (s *PlatformPlayerStorage) persistExpiry(ctx context.Context, b data.Binding) {
	bounded, cancel := context.WithTimeout(context.WithoutCancel(ctx), 3*time.Second)
	defer cancel()
	_ = s.Transact(bounded, func(tx player.Transaction) error {
		if e := tx.Barrier(bounded, b); e != nil {
			return e
		}
		state, e := tx.State(bounded, b)
		if e != nil {
			return e
		}
		v := state.StorageValue()
		leases, e := tx.Leases(bounded, b)
		if e != nil {
			return e
		}
		now, e := tx.Now(bounded)
		if e != nil {
			return e
		}
		if !v.Paused && player.Disconnected(v, leases, now) {
			state, e = player.PauseDisconnected(v)
			if e != nil {
				return e
			}
			return tx.PutState(bounded, state)
		}
		return nil
	})
}
func (r *playerSessionRepository) Begin(ctx context.Context, h data.Header) (data.Transaction, error) {
	if r.state() == nil {
		return nil, data.ErrDenied
	}
	tx, e := r.state().base.Begin(ctx, h)
	if e != nil || h.ReadOnly {
		return tx, e
	}
	wrapped, ok := tx.(*platformLaunchHostTransaction)
	if !ok {
		_ = tx.Rollback()
		return nil, data.ErrDenied
	}
	native, ok := wrapped.state().tx.(*hostTransaction)
	if !ok {
		_ = tx.Rollback()
		return nil, data.ErrDenied
	}
	if e = nativePlayerReady(ctx, native.tx, h); e != nil {
		_ = tx.Rollback()
		if e == player.ErrConnectionExpired {
			r.state().player.persistExpiry(ctx, h.Binding)
		}
		return nil, data.ErrDenied
	}
	d := &playerNativeTxData{base: tx, sql: native.tx, player: r.state().player, header: h}
	return &playerNativeTransaction{data: &d}, nil
}

type playerNativeTransaction struct{ data **playerNativeTxData }
type playerNativeTxData struct {
	base   data.Transaction
	sql    *sql.Tx
	player *PlatformPlayerStorage
	header data.Header
}

func (playerNativeTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<controlled native transaction>")
}
func (playerNativeTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t *playerNativeTransaction) state() *playerNativeTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func (t *playerNativeTransaction) Snapshot() data.Snapshot { return t.state().base.Snapshot() }
func (t *playerNativeTransaction) Rollback() error         { return t.state().base.Rollback() }
func (t *playerNativeTransaction) Commit(ctx context.Context, c data.Commit) (data.Receipt, error) {
	d := t.state()
	if d == nil || c.Header != d.header || c.Header.ExpectedVersion >= math.MaxInt64 {
		return data.Receipt{}, data.ErrDenied
	}
	if e := nativePlayerReady(ctx, d.sql, d.header); e != nil {
		_ = d.base.Rollback()
		if e == player.ErrConnectionExpired {
			d.player.persistExpiry(ctx, d.header.Binding)
		}
		return data.Receipt{}, data.ErrDenied
	}
	return d.base.Commit(ctx, c)
}

var _ player.Storage = (*PlatformPlayerStorage)(nil)
var _ launch.Storage = (*ControlledLaunchStorage)(nil)

// The controlled queue retains paused jobs without allocating a new execution
// or delivery attempt. Immutable source/worker/receipt checks are the original
// B006 algorithms; only the current player gate is interposed before a write.
type ControlledPlayerTasks struct{ data **controlledPlayerTaskData }
type controlledPlayerTaskData struct {
	base   *PlatformTaskStorage
	player *PlatformPlayerStorage
}

func (ControlledPlayerTasks) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<controlled task storage>")
}
func (ControlledPlayerTasks) MarshalJSON() ([]byte, error) { return nil, task.ErrDenied }
func (s *ControlledPlayerTasks) state() *controlledPlayerTaskData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func NewControlledPlayerTasks(base *PlatformTaskStorage, p *PlatformPlayerStorage) (*ControlledPlayerTasks, error) {
	if base.state() == nil || p.state() == nil || base.state().repo != p.state().repo {
		return nil, task.ErrInvalid
	}
	d := &controlledPlayerTaskData{base, p}
	return &ControlledPlayerTasks{data: &d}, nil
}
func controlledTaskReady(ctx context.Context, tx *sql.Tx, v task.JobData) error {
	t := playerSQL(tx)
	state, e := t.State(ctx, v.Binding)
	if e != nil {
		return task.ErrDenied
	}
	s := state.StorageValue()
	if s.Scope != v.Scope || s.ConfigurationHash != v.ConfigurationHash {
		return task.ErrDenied
	}
	leases, e := t.Leases(ctx, v.Binding)
	if e != nil {
		return task.ErrUnavailable
	}
	now, e := t.Now(ctx)
	if e != nil {
		return task.ErrUnavailable
	}
	if !s.Paused && player.Disconnected(s, leases, now) {
		state, e = player.PauseDisconnected(s)
		if e != nil || t.PutState(ctx, state) != nil {
			return task.ErrUnavailable
		}
		s = state.StorageValue()
	}
	if s.Paused {
		return player.ErrPaused
	}
	return nil
}
func (s *ControlledPlayerTasks) SaveResult(ctx context.Context, w task.Worker, j task.Job, v task.Value, i task.Inputs) (task.Job, error) {
	return s.state().base.SaveResult(ctx, w, j, v, i)
}
func (s *ControlledPlayerTasks) Current(ctx context.Context, w task.Worker, j task.Job) (task.Job, error) {
	v, e := j.StorageValue()
	if e != nil {
		return task.Job{}, task.ErrDenied
	}
	allowed := false
	e = s.state().player.Transact(ctx, func(t player.Transaction) error {
		state, e := t.State(ctx, v.Binding)
		if e != nil {
			return auth.ErrDenied
		}
		if state.StorageValue().Scope != v.Scope {
			return auth.ErrDenied
		}
		leases, e := t.Leases(ctx, v.Binding)
		if e != nil {
			return auth.ErrUnavailable
		}
		now, e := t.Now(ctx)
		if e != nil {
			return auth.ErrUnavailable
		}
		x := state.StorageValue()
		if !x.Paused && player.Disconnected(x, leases, now) {
			state, e = player.PauseDisconnected(x)
			if e != nil {
				return e
			}
			if e = t.PutState(ctx, state); e != nil {
				return e
			}
			x = state.StorageValue()
		}
		allowed = !x.Paused
		return nil
	})
	if e != nil || !allowed {
		return task.Job{}, task.ErrDenied
	}
	return s.state().base.Current(ctx, w, j)
}
func (s *ControlledPlayerTasks) Complete(c context.Context, w task.Worker, j task.Job, r data.Receipt) error {
	return s.state().base.Complete(c, w, j, r)
}
func (s *ControlledPlayerTasks) Cancel(c context.Context, w task.Worker, b data.Binding, id string) error {
	return s.state().base.Cancel(c, w, b, id)
}
func (s *ControlledPlayerTasks) Status(c context.Context, w task.Worker, b data.Binding, id string) (string, error) {
	return s.state().base.Status(c, w, b, id)
}

var _ task.Storage = (*ControlledPlayerTasks)(nil)

func (s *ControlledPlayerTasks) Claim(ctx context.Context, w task.Worker) (task.Job, error) {
	tx, now, e := s.state().base.begin(ctx, w)
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
			n, e := s.state().base.seed(ctx, tx, space, now, budget)
			if e != nil {
				return task.Job{}, e
			}
			budget -= n
		}
		blocked := []string{}
		for visited := 0; visited < 64; visited++ {
			r, e := scanTask(tx.QueryRowContext(ctx, `SELECT `+taskColumns+` FROM platform_task.jobs WHERE workspace=$1 AND (status IN ('queued','ready') OR (status IN ('running','delivering') AND lease_until<=$2)) AND NOT (session||'/'||task_id=ANY($3)) ORDER BY expires,session,task_id LIMIT 1 FOR UPDATE SKIP LOCKED`, space, now, blocked))
			if errors.Is(e, task.ErrNotFound) {
				break
			}
			if e != nil {
				return task.Job{}, e
			}
			v := r.job
			if e = controlledTaskReady(ctx, tx, v); e == player.ErrPaused {
				blocked = append(blocked, v.Binding.Session+"/"+v.TaskID)
				continue
			} else if e != nil {
				return task.Job{}, task.SafeError(e)
			}
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
			v.LeaseUntil = now.Add(s.state().base.state().lease)
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
			if e = s.state().base.commit(ctx, tx, w, space, "claim"); e != nil {
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

func (s *ControlledPlayerTasks) Retry(ctx context.Context, w task.Worker, j task.Job) error {
	tx, now, e := s.state().base.begin(ctx, w)
	if e != nil {
		return e
	}
	defer tx.Rollback()
	r, e := leaseTask(ctx, tx, w, j, now, true)
	if e != nil {
		return e
	}
	v := r.job
	if e = controlledTaskReady(ctx, tx, v); e == player.ErrPaused {
		if e = tx.Commit(); e != nil {
			return taskSQL(e)
		}
		return task.ErrDenied
	} else if e != nil {
		return task.SafeError(e)
	}
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
	return s.state().base.commit(ctx, tx, w, v.Binding.Workspace, "retry")
}
