// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/session/persistence"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"io"
)

type PlatformLaunchStorage struct{ data **platformLaunchData }
type platformLaunchData struct {
	repo *PlatformAuthRepository
	host *HostRepository
}
type platformLaunchTransaction struct{ data **platformLaunchTxData }
type platformLaunchTxData struct {
	core *platformCoreTransaction
	host *HostRepository
}

func (*PlatformLaunchStorage) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<launch storage>")
}
func (*PlatformLaunchStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (*platformLaunchTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<launch transaction>")
}
func (*platformLaunchTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *PlatformLaunchStorage) state() *platformLaunchData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func (t *platformLaunchTransaction) state() *platformLaunchTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func NewPlatformLaunchStorage(repo *PlatformAuthRepository) (*PlatformLaunchStorage, error) {
	if repo == nil || repo.state() == nil {
		return nil, auth.ErrInvalid
	}
	h, e := NewHostRepository(HostOptions{DB: repo.state().core.state().db})
	if e != nil {
		return nil, auth.ErrUnavailable
	}
	d := &platformLaunchData{repo: repo, host: h}
	return &PlatformLaunchStorage{data: &d}, nil
}
func (s *PlatformLaunchStorage) RuntimeRepository() persistence.Repository {
	if s.state() == nil {
		return nil
	}
	d := &platformLaunchRuntimeData{host: s.state().host}
	return &platformLaunchRuntime{data: &d}
}
func (s *PlatformLaunchStorage) SessionRepository() install.SessionRepository {
	if s.state() == nil {
		return nil
	}
	d := &platformLaunchSessionData{host: s.state().host}
	return &platformLaunchSessionRepository{data: &d}
}
func (s *PlatformLaunchStorage) Bind(tx core.Transaction) (launch.Transaction, error) {
	if s.state() == nil {
		return nil, auth.ErrUnavailable
	}
	if _, e := auth.RoomAdmissionSession(tx); e != nil {
		return nil, auth.ErrDenied
	}
	c, ok := auth.RoomStorageCore(tx).(*platformCoreTransaction)
	if !ok || c == nil || c.tx == nil {
		return nil, auth.ErrDenied
	}
	d := &platformLaunchTxData{core: c, host: s.state().host}
	return &platformLaunchTransaction{data: &d}, nil
}
func (t *platformLaunchTransaction) Preparation(ctx context.Context, s core.Scope) (launch.Preparation, error) {
	var raw []byte
	var id, hash, ch string
	var rev uint64
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT body,configuration_id,configuration_hash,graph_hash,revision FROM platform_launch.preparation WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID).Scan(&raw, &id, &ch, &hash, &rev)
	if e != nil {
		return launch.Preparation{}, authStorageError(e)
	}
	var v launch.PreparationData
	if checkpoint.StrictDecode(raw, &v, 16384) != nil || v.Scope != s || v.ConfigurationID != id || v.ConfigurationHash != ch || v.GraphHash != hash || v.Revision != rev {
		return launch.Preparation{}, auth.ErrDenied
	}
	return auth.RoomSecret(v), nil
}
func (t *platformLaunchTransaction) PutPreparation(ctx context.Context, v launch.Preparation) error {
	d := v.StorageValue()
	raw, e := json.Marshal(d)
	if e != nil || len(raw) > 16384 {
		return auth.ErrInvalid
	}
	tx := t.state().core.tx
	if _, e = tx.ExecContext(ctx, `DELETE FROM platform_launch.acknowledgments WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID); e != nil {
		return authStorageError(e)
	}
	_, e = tx.ExecContext(ctx, `INSERT INTO platform_launch.preparation(workspace_id,room_id,game_id,revision,graph_hash,configuration_id,configuration_hash,body) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(workspace_id,room_id,game_id) DO UPDATE SET revision=EXCLUDED.revision,graph_hash=EXCLUDED.graph_hash,configuration_id=EXCLUDED.configuration_id,configuration_hash=EXCLUDED.configuration_hash,body=EXCLUDED.body`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.Revision, d.GraphHash, d.ConfigurationID, d.ConfigurationHash, raw)
	if e != nil {
		return authStorageError(e)
	}
	return auth.SafeError(t.state().core.inject(ctx, "launch-after-preparation"))
}
func (t *platformLaunchTransaction) Acknowledgments(ctx context.Context, s core.Scope) ([]launch.Acknowledgment, error) {
	rows, e := t.state().core.tx.QueryContext(ctx, `SELECT participant_id,revision,configuration_hash,graph_hash,body FROM platform_launch.acknowledgments WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 ORDER BY participant_id LIMIT 65 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID)
	if e != nil {
		return nil, authStorageError(e)
	}
	defer rows.Close()
	out := []launch.Acknowledgment{}
	for rows.Next() {
		var participant, hash, ch string
		var revision uint64
		var raw []byte
		if e = rows.Scan(&participant, &revision, &ch, &hash, &raw); e != nil {
			return nil, authStorageError(e)
		}
		var v launch.AcknowledgmentData
		if checkpoint.StrictDecode(raw, &v, 16384) != nil || v.Scope != s || v.ParticipantID != participant || v.Revision != revision || v.GraphHash != hash || v.ConfigurationHash != ch {
			return nil, auth.ErrDenied
		}
		out = append(out, auth.RoomSecret(v))
	}
	if len(out) > 64 {
		return nil, auth.ErrConflict
	}
	return out, authStorageError(rows.Err())
}
func (t *platformLaunchTransaction) PutAcknowledgment(ctx context.Context, v launch.Acknowledgment) error {
	d := v.StorageValue()
	raw, e := json.Marshal(d)
	if e != nil || len(raw) > 16384 {
		return auth.ErrInvalid
	}
	_, e = t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_launch.acknowledgments(workspace_id,room_id,game_id,participant_id,revision,configuration_hash,graph_hash,body) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(workspace_id,room_id,game_id,participant_id) DO UPDATE SET revision=EXCLUDED.revision,configuration_hash=EXCLUDED.configuration_hash,graph_hash=EXCLUDED.graph_hash,body=EXCLUDED.body`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.ParticipantID, d.Revision, d.ConfigurationHash, d.GraphHash, raw)
	if e != nil {
		return authStorageError(e)
	}
	return auth.SafeError(t.state().core.inject(ctx, "launch-after-acknowledgment"))
}
func (t *platformLaunchTransaction) Session(ctx context.Context, s core.Scope) (launch.Session, error) {
	v := launch.SessionData{Scope: s}
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT configuration_id,configuration_hash,revision,workspace_id,session_id,graph_hash FROM platform_launch.sessions WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID).Scan(&v.ConfigurationID, &v.ConfigurationHash, &v.Revision, &v.Binding.Workspace, &v.Binding.Session, &v.Binding.GraphHash)
	if e != nil {
		return launch.Session{}, authStorageError(e)
	}
	if v.Binding.Workspace != s.WorkspaceID || !validBinding(v.Binding) {
		return launch.Session{}, auth.ErrDenied
	}
	return auth.RoomSecret(v), nil
}
func (t *platformLaunchTransaction) PutSession(ctx context.Context, v launch.Session) error {
	d := v.StorageValue()
	if d.Binding.Workspace != d.Scope.WorkspaceID || !validBinding(d.Binding) {
		return auth.ErrDenied
	}
	tx := t.state().core.tx
	_, e := tx.ExecContext(ctx, `INSERT INTO platform_launch.sessions(workspace_id,room_id,game_id,session_id,graph_hash,configuration_id,configuration_hash,revision) VALUES($1,$2,$3,$4,$5,$6,$7,$8)`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.Binding.Session, d.Binding.GraphHash, d.ConfigurationID, d.ConfigurationHash, d.Revision)
	if e != nil {
		return authStorageError(e)
	}
	r, e := tx.ExecContext(ctx, `UPDATE platform_room.rooms SET state='launched' WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND state='lobby'`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID)
	if e != nil {
		return authStorageError(e)
	}
	n, e := r.RowsAffected()
	if e != nil || n != 1 {
		return auth.ErrConflict
	}
	return auth.SafeError(t.state().core.inject(ctx, "launch-after-room-transition"))
}
func (t *platformLaunchTransaction) SessionRepository(s core.Scope, b data.Binding) (install.SessionRepository, error) {
	if t.state() == nil || b.Workspace != s.WorkspaceID || !validBinding(b) {
		return nil, auth.ErrDenied
	}
	d := &platformLaunchSessionData{core: t.state().core, host: t.state().host, scope: s, binding: b}
	return &platformLaunchSessionRepository{data: &d}, nil
}
