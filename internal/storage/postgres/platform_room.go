// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package postgres

import (
	"context"
	"database/sql"
	"fmt"
	"io"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
)

type PlatformRoomStorage struct{ data **platformRoomData }
type platformRoomData struct{ repo *PlatformAuthRepository }

func NewPlatformRoomStorage(repo *PlatformAuthRepository) (*PlatformRoomStorage, error) {
	if repo == nil || repo.state() == nil {
		return nil, auth.ErrInvalid
	}
	d := &platformRoomData{repo: repo}
	return &PlatformRoomStorage{data: &d}, nil
}
func (r *PlatformRoomStorage) state() *platformRoomData {
	if r == nil || r.data == nil {
		return nil
	}
	return *r.data
}
func (*PlatformRoomStorage) Format(f fmt.State, _ rune)   { _, _ = io.WriteString(f, "<room storage>") }
func (*PlatformRoomStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (r *PlatformRoomStorage) Bind(tx core.Transaction) (room.Transaction, error) {
	if r.state() == nil {
		return nil, auth.ErrUnavailable
	}
	c, ok := auth.RoomStorageCore(tx).(*platformCoreTransaction)
	if !ok || c == nil || c.tx == nil {
		return nil, auth.ErrDenied
	}
	d := &platformRoomTxData{core: c}
	return &platformRoomTransaction{data: &d}, nil
}

type platformRoomTransaction struct{ data **platformRoomTxData }
type platformRoomTxData struct{ core *platformCoreTransaction }

func (t *platformRoomTransaction) state() *platformRoomTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func (*platformRoomTransaction) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<room transaction>")
}
func (*platformRoomTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (t *platformRoomTransaction) db() *sql.Tx                { return t.state().core.tx }
func (t *platformRoomTransaction) inject(ctx context.Context, point string) error {
	return auth.SafeError(t.state().core.inject(ctx, point))
}

func (t *platformRoomTransaction) Room(ctx context.Context, w, id string) (room.StoredRoom, error) {
	var d room.RoomData
	e := t.db().QueryRowContext(ctx, `SELECT workspace_id,room_id,game_id,name,owner_account_id,state FROM platform_room.rooms WHERE workspace_id=$1 AND room_id=$2 FOR UPDATE`, w, id).Scan(&d.Scope.WorkspaceID, &d.Scope.RoomID, &d.Scope.GameID, &d.Name, &d.Owner, &d.State)
	d.ID = d.Scope.RoomID
	return auth.RoomSecret(d), authStorageError(e)
}
func (t *platformRoomTransaction) InsertRoom(ctx context.Context, v room.StoredRoom) error {
	d := v.StorageValue()
	if d.ID != d.Scope.RoomID || d.State != "lobby" {
		return auth.ErrDenied
	}
	_, e := t.db().ExecContext(ctx, `INSERT INTO platform_room.rooms(workspace_id,room_id,game_id,name,owner_account_id,state) VALUES($1,$2,$3,$4,$5,'lobby')`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.Name, d.Owner)
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-room")
}
func (t *platformRoomTransaction) CloseRoom(ctx context.Context, s core.Scope) error {
	for _, q := range []string{
		`UPDATE platform_room.rooms SET state='closed' WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND state='lobby'`,
		`UPDATE platform_room.invitations SET revoked=true WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3`,
		`UPDATE platform_room.admissions SET status='expired' WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3`,
		`UPDATE platform_room.participants SET active=false,host=false WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3`,
		`DELETE FROM platform_room.managers WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3`,
		`UPDATE platform_core.guests SET disabled=true WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3`,
	} {
		if _, e := t.db().ExecContext(ctx, q, s.WorkspaceID, s.RoomID, s.GameID); e != nil {
			return authStorageError(e)
		}
	}
	return t.inject(ctx, "after-room-close")
}

const invitationColumns = `workspace_id,room_id,game_id,id,link_hash,code_hash,approval_required,revoked,expires_at,max_uses,uses`

type rowScanner interface{ Scan(...any) error }

func scanInvitation(row rowScanner) (room.Invitation, error) {
	var d room.InvitationData
	e := row.Scan(&d.Scope.WorkspaceID, &d.Scope.RoomID, &d.Scope.GameID, &d.ID, &d.LinkHash, &d.CodeHash, &d.ApprovalRequired, &d.Revoked, &d.ExpiresAt, &d.MaxUses, &d.Uses)
	return auth.RoomSecret(d), authStorageError(e)
}
func (t *platformRoomTransaction) Invitation(ctx context.Context, s core.Scope, id string) (room.Invitation, error) {
	return scanInvitation(t.db().QueryRowContext(ctx, `SELECT `+invitationColumns+` FROM platform_room.invitations WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND id=$4 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID, id))
}
func (t *platformRoomTransaction) InvitationBySecret(ctx context.Context, kind, hash string) (room.Invitation, error) {
	q := `SELECT ` + invitationColumns + ` FROM platform_room.invitations WHERE link_hash=$1 FOR UPDATE`
	if kind == "code" {
		q = `SELECT ` + invitationColumns + ` FROM platform_room.invitations WHERE code_hash=$1 FOR UPDATE`
	} else if kind != "link" {
		return room.Invitation{}, auth.ErrDenied
	}
	return scanInvitation(t.db().QueryRowContext(ctx, q, hash))
}
func (t *platformRoomTransaction) PutInvitation(ctx context.Context, v room.Invitation) error {
	d := v.StorageValue()
	_, e := t.db().ExecContext(ctx, `INSERT INTO platform_room.invitations(workspace_id,room_id,game_id,id,link_hash,code_hash,approval_required,revoked,expires_at,max_uses,uses) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(workspace_id,room_id,game_id,id) DO UPDATE SET uses=EXCLUDED.uses,revoked=EXCLUDED.revoked`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.ID, d.LinkHash, d.CodeHash, d.ApprovalRequired, d.Revoked, d.ExpiresAt, d.MaxUses, d.Uses)
	if e != nil {
		return authStorageError(e)
	}
	if d.Revoked {
		_, e = t.db().ExecContext(ctx, `UPDATE platform_room.admissions SET status='expired' WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND invitation_id=$4 AND (status='pending' OR (mode='guest' AND NOT token_used))`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.ID)
		if e != nil {
			return authStorageError(e)
		}
	}
	return t.inject(ctx, "after-room-invitation")
}

const admissionColumns = `id,workspace_id,room_id,game_id,invitation_id,mode,status,name,COALESCE(owner_account_id,''),COALESCE(owner_session_hash,''),COALESCE(participant_id,''),expires_at,COALESCE(token_hash,''),token_expires_at,token_used`

func scanAdmission(row rowScanner) (room.Admission, error) {
	var d room.AdmissionData
	var expiry sql.NullTime
	e := row.Scan(&d.ID, &d.Scope.WorkspaceID, &d.Scope.RoomID, &d.Scope.GameID, &d.InvitationID, &d.Mode, &d.Status, &d.Name, &d.OwnerAccount, &d.OwnerSession, &d.ParticipantID, &d.ExpiresAt, &d.TokenHash, &expiry, &d.TokenUsed)
	if expiry.Valid {
		d.TokenExpiresAt = expiry.Time
	}
	return auth.RoomSecret(d), authStorageError(e)
}
func (t *platformRoomTransaction) Admission(ctx context.Context, id string) (room.Admission, error) {
	return scanAdmission(t.db().QueryRowContext(ctx, `SELECT `+admissionColumns+` FROM platform_room.admissions WHERE id=$1 FOR UPDATE`, id))
}
func (t *platformRoomTransaction) AdmissionByToken(ctx context.Context, hash string) (room.Admission, error) {
	return scanAdmission(t.db().QueryRowContext(ctx, `SELECT `+admissionColumns+` FROM platform_room.admissions WHERE token_hash=$1 FOR UPDATE`, hash))
}
func (t *platformRoomTransaction) ActorAdmission(ctx context.Context, s core.Scope, invite, mode, owner string) (room.Admission, error) {
	q := `SELECT ` + admissionColumns + ` FROM platform_room.admissions WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND invitation_id=$4 AND mode='account' AND owner_account_id=$5 FOR UPDATE`
	if mode == "guest" {
		q = `SELECT ` + admissionColumns + ` FROM platform_room.admissions WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND invitation_id=$4 AND mode='guest' AND owner_session_hash=$5 FOR UPDATE`
	} else if mode != "account" {
		return room.Admission{}, auth.ErrDenied
	}
	return scanAdmission(t.db().QueryRowContext(ctx, q, s.WorkspaceID, s.RoomID, s.GameID, invite, owner))
}
func nullText(v string) any {
	if v == "" {
		return nil
	}
	return v
}
func (t *platformRoomTransaction) PutAdmission(ctx context.Context, v room.Admission) error {
	d := v.StorageValue()
	var expiry any
	if !d.TokenExpiresAt.IsZero() {
		expiry = d.TokenExpiresAt
	}
	_, e := t.db().ExecContext(ctx, `INSERT INTO platform_room.admissions(id,workspace_id,room_id,game_id,invitation_id,mode,status,name,owner_account_id,owner_session_hash,participant_id,expires_at,token_hash,token_expires_at,token_used) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status,participant_id=EXCLUDED.participant_id,token_hash=EXCLUDED.token_hash,token_expires_at=EXCLUDED.token_expires_at,token_used=EXCLUDED.token_used`, d.ID, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.InvitationID, d.Mode, d.Status, d.Name, nullText(d.OwnerAccount), nullText(d.OwnerSession), nullText(d.ParticipantID), d.ExpiresAt, nullText(d.TokenHash), expiry, d.TokenUsed)
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-room-admission")
}
func (t *platformRoomTransaction) Pending(ctx context.Context, s core.Scope, now time.Time) ([]room.Admission, error) {
	rows, e := t.db().QueryContext(ctx, `SELECT a.`+`id,a.workspace_id,a.room_id,a.game_id,a.invitation_id,a.mode,a.status,a.name,COALESCE(a.owner_account_id,''),COALESCE(a.owner_session_hash,''),COALESCE(a.participant_id,''),a.expires_at,COALESCE(a.token_hash,''),a.token_expires_at,a.token_used FROM platform_room.admissions a JOIN platform_room.invitations i ON (i.workspace_id,i.room_id,i.game_id,i.id)=(a.workspace_id,a.room_id,a.game_id,a.invitation_id) WHERE a.workspace_id=$1 AND a.room_id=$2 AND a.game_id=$3 AND a.status='pending' AND a.expires_at>$4 AND i.expires_at>$4 AND NOT i.revoked ORDER BY a.id LIMIT 64`, s.WorkspaceID, s.RoomID, s.GameID, now)
	if e != nil {
		return nil, authStorageError(e)
	}
	defer rows.Close()
	out := []room.Admission{}
	for rows.Next() {
		v, e := scanAdmission(rows)
		if e != nil {
			return nil, e
		}
		out = append(out, v)
	}
	return out, authStorageError(rows.Err())
}

const participantColumns = `workspace_id,room_id,game_id,id,COALESCE(account_id,''),COALESCE(guest_id,''),name,active,host`

func scanParticipant(row rowScanner) (room.Participant, error) {
	var d room.ParticipantData
	e := row.Scan(&d.Scope.WorkspaceID, &d.Scope.RoomID, &d.Scope.GameID, &d.ID, &d.AccountID, &d.GuestID, &d.Name, &d.Active, &d.Host)
	return auth.RoomSecret(d), authStorageError(e)
}
func (t *platformRoomTransaction) Participant(ctx context.Context, s core.Scope, id string) (room.Participant, error) {
	return scanParticipant(t.db().QueryRowContext(ctx, `SELECT `+participantColumns+` FROM platform_room.participants WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND id=$4 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID, id))
}
func (t *platformRoomTransaction) AccountParticipant(ctx context.Context, s core.Scope, id string) (room.Participant, error) {
	return scanParticipant(t.db().QueryRowContext(ctx, `SELECT `+participantColumns+` FROM platform_room.participants WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND account_id=$4 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID, id))
}
func (t *platformRoomTransaction) GuestParticipant(ctx context.Context, s core.Scope, id string) (room.Participant, error) {
	return scanParticipant(t.db().QueryRowContext(ctx, `SELECT `+participantColumns+` FROM platform_room.participants WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND guest_id=$4 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID, id))
}
func (t *platformRoomTransaction) Participants(ctx context.Context, s core.Scope) ([]room.Participant, error) {
	rows, e := t.db().QueryContext(ctx, `SELECT `+participantColumns+` FROM platform_room.participants WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND active ORDER BY id LIMIT 65 FOR UPDATE`, s.WorkspaceID, s.RoomID, s.GameID)
	if e != nil {
		return nil, authStorageError(e)
	}
	defer rows.Close()
	out := []room.Participant{}
	for rows.Next() {
		v, e := scanParticipant(rows)
		if e != nil {
			return nil, e
		}
		out = append(out, v)
	}
	if len(out) > 64 {
		return nil, auth.ErrConflict
	}
	return out, authStorageError(rows.Err())
}
func (t *platformRoomTransaction) PutParticipant(ctx context.Context, v room.Participant) error {
	d := v.StorageValue()
	_, e := t.db().ExecContext(ctx, `INSERT INTO platform_room.participants(workspace_id,room_id,game_id,id,account_id,guest_id,name,active,host) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(workspace_id,room_id,game_id,id) DO UPDATE SET account_id=EXCLUDED.account_id,name=EXCLUDED.name,active=EXCLUDED.active,host=EXCLUDED.host`, d.Scope.WorkspaceID, d.Scope.RoomID, d.Scope.GameID, d.ID, nullText(d.AccountID), nullText(d.GuestID), d.Name, d.Active, d.Host)
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-room-participant")
}
func (t *platformRoomTransaction) Manager(ctx context.Context, s core.Scope, account string) (bool, error) {
	var found bool
	e := t.db().QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM platform_room.managers WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND account_id=$4)`, s.WorkspaceID, s.RoomID, s.GameID, account).Scan(&found)
	return found, authStorageError(e)
}
func (t *platformRoomTransaction) SetManager(ctx context.Context, s core.Scope, account string, enabled bool) error {
	q := `DELETE FROM platform_room.managers WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND account_id=$4`
	if enabled {
		q = `INSERT INTO platform_room.managers(workspace_id,room_id,game_id,account_id) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING`
	}
	_, e := t.db().ExecContext(ctx, q, s.WorkspaceID, s.RoomID, s.GameID, account)
	if e != nil {
		return authStorageError(e)
	}
	return t.inject(ctx, "after-room-manager")
}
