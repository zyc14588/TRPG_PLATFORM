// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package postgres

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"io"
	"time"
)

type PlatformModelStorage struct{ data **platformModelData }
type platformModelData struct{ repo *PlatformAuthRepository }
type platformModelTransaction struct{ data **platformModelTxData }
type platformModelTxData struct{ core *platformCoreTransaction }

func (*PlatformModelStorage) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<private model storage>")
}
func (*PlatformModelStorage) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (*platformModelTransaction) Format(s fmt.State, _ rune) {
	_, _ = io.WriteString(s, "<private model transaction>")
}
func (*platformModelTransaction) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *PlatformModelStorage) state() *platformModelData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func (t *platformModelTransaction) state() *platformModelTxData {
	if t == nil || t.data == nil {
		return nil
	}
	return *t.data
}
func NewPlatformModelStorage(repo *PlatformAuthRepository) (*PlatformModelStorage, error) {
	if repo == nil || repo.state() == nil {
		return nil, auth.ErrInvalid
	}
	d := &platformModelData{repo: repo}
	return &PlatformModelStorage{data: &d}, nil
}
func (s *PlatformModelStorage) Bind(tx core.Transaction) (model.Transaction, error) {
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
	d := &platformModelTxData{core: c}
	return &platformModelTransaction{data: &d}, nil
}
func (t *platformModelTransaction) Credential(ctx context.Context, sc core.Scope, seat, id string) (credential.Record, error) {
	b := credential.Binding{Scope: sc, SeatID: seat, ID: id}
	var expires sql.NullTime
	var raw []byte
	var revoked bool
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT owner_kind,owner_id,lifetime,expires_at,version,ciphertext,revoked FROM platform_model.credentials WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND seat_id=$4 AND id=$5 FOR UPDATE`, sc.WorkspaceID, sc.RoomID, sc.GameID, seat, id).Scan(&b.OwnerKind, &b.OwnerID, &b.Lifetime, &expires, &b.Version, &raw, &revoked)
	if e != nil {
		return credential.Record{}, authStorageError(e)
	}
	if expires.Valid {
		b.ExpiresAt = expires.Time.UTC().Truncate(time.Microsecond)
	}
	if !credential.ValidBinding(b) || len(raw) < 29 || len(raw) > credential.MaxKeyBytes+28 {
		return credential.Record{}, auth.ErrDenied
	}
	return credential.StoredRecord(credential.RecordData{Binding: b, Ciphertext: raw, Revoked: revoked}), nil
}
func (t *platformModelTransaction) InsertCredential(ctx context.Context, record credential.Record) error {
	v := credential.RecordValue(record)
	b := v.Binding
	if !credential.ValidBinding(b) || v.Revoked || b.Version != 1 || len(v.Ciphertext) < 29 || len(v.Ciphertext) > credential.MaxKeyBytes+28 {
		return auth.ErrInvalid
	}
	var account, guest, expiry any
	if b.OwnerKind == "account" {
		account = b.OwnerID
	} else {
		guest = b.OwnerID
	}
	if !b.ExpiresAt.IsZero() {
		expiry = b.ExpiresAt
	}
	_, e := t.state().core.tx.ExecContext(ctx, `INSERT INTO platform_model.credentials(workspace_id,room_id,game_id,seat_id,id,owner_kind,owner_id,owner_account_id,owner_guest_id,lifetime,expires_at,version,ciphertext) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)`, b.Scope.WorkspaceID, b.Scope.RoomID, b.Scope.GameID, b.SeatID, b.ID, b.OwnerKind, b.OwnerID, account, guest, b.Lifetime, expiry, b.Version, v.Ciphertext)
	if e != nil {
		return authStorageError(e)
	}
	return auth.SafeError(t.state().core.inject(ctx, "model-after-credential"))
}
func (t *platformModelTransaction) RevokeCredential(ctx context.Context, sc core.Scope, seat, id string, version uint64) error {
	return auth.SafeError(platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_model.credentials SET revoked=true WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND seat_id=$4 AND id=$5 AND version=$6 AND NOT revoked`, sc.WorkspaceID, sc.RoomID, sc.GameID, seat, id, version)))
}
func modelTupleHash(v model.Tuple) string {
	b, _ := json.Marshal(v)
	h := sha256.Sum256(b)
	return hex.EncodeToString(h[:])
}
func (t *platformModelTransaction) Configuration(ctx context.Context, sc core.Scope, seat, id string) (model.Configuration, error) {
	var raw []byte
	var version, credVersion, prepVersion uint64
	var credentialID, certID, certHash, tupleHash, configID, configHash, graphHash, ownerKind, ownerID string
	var revoked bool
	e := t.state().core.tx.QueryRowContext(ctx, `SELECT version,credential_id,credential_version,primary_id,primary_hash,tuple_hash,configuration_id,configuration_hash,graph_hash,preparation_revision,owner_kind,owner_id,body,revoked FROM platform_model.configurations WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND seat_id=$4 AND selection=$5 FOR UPDATE`, sc.WorkspaceID, sc.RoomID, sc.GameID, seat, id).Scan(&version, &credentialID, &credVersion, &certID, &certHash, &tupleHash, &configID, &configHash, &graphHash, &prepVersion, &ownerKind, &ownerID, &raw, &revoked)
	if e != nil {
		return model.Configuration{}, authStorageError(e)
	}
	var v model.ConfigurationData
	if checkpoint.StrictDecode(raw, &v, 16_384) != nil || v.Scope != sc || v.SeatID != seat || v.Selection != id || v.Version != version || v.CredentialID != credentialID || v.CredentialVersion != credVersion || v.Primary.ID != certID || v.Primary.Hash != certHash || modelTupleHash(v.Tuple) != tupleHash || v.ConfigurationID != configID || v.ConfigurationHash != configHash || v.GraphHash != graphHash || v.PreparationRevision != prepVersion || v.OwnerKind != ownerKind || v.OwnerID != ownerID || v.Revoked || len(v.Fallbacks) > 4 {
		return model.Configuration{}, auth.ErrDenied
	}
	v.Revoked = revoked
	return model.CopyConfiguration(auth.RoomSecret(v)), nil
}
func (t *platformModelTransaction) PutConfiguration(ctx context.Context, value model.Configuration, expected uint64) error {
	v := model.CopyConfiguration(value).StorageValue()
	if v.Version != expected+1 || expected >= 1<<53 || v.Revoked || len(v.Fallbacks) > 4 {
		return auth.ErrInvalid
	}
	raw, e := json.Marshal(v)
	if e != nil || len(raw) > 16_384 {
		return auth.ErrInvalid
	}
	tx := t.state().core.tx
	args := []any{v.Scope.WorkspaceID, v.Scope.RoomID, v.Scope.GameID, v.SeatID, v.Selection, v.Version, v.CredentialID, v.CredentialVersion, v.Primary.ID, v.Primary.Hash, modelTupleHash(v.Tuple), v.ConfigurationID, v.ConfigurationHash, v.GraphHash, v.PreparationRevision, v.OwnerKind, v.OwnerID, raw}
	if expected == 0 {
		_, e = tx.ExecContext(ctx, `INSERT INTO platform_model.configurations(workspace_id,room_id,game_id,seat_id,selection,version,credential_id,credential_version,primary_id,primary_hash,tuple_hash,configuration_id,configuration_hash,graph_hash,preparation_revision,owner_kind,owner_id,body) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)`, args...)
		if e != nil {
			return authStorageError(e)
		}
	} else {
		args = append(args, expected)
		e = platformAffected(tx.ExecContext(ctx, `UPDATE platform_model.configurations SET version=$6,credential_id=$7,credential_version=$8,primary_id=$9,primary_hash=$10,tuple_hash=$11,configuration_id=$12,configuration_hash=$13,graph_hash=$14,preparation_revision=$15,owner_kind=$16,owner_id=$17,body=$18 WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND seat_id=$4 AND selection=$5 AND version=$19 AND NOT revoked`, args...))
		if e != nil {
			return auth.SafeError(e)
		}
	}
	return auth.SafeError(t.state().core.inject(ctx, "model-after-configuration"))
}
func (t *platformModelTransaction) RevokeConfiguration(ctx context.Context, sc core.Scope, seat, id string, version uint64) error {
	return auth.SafeError(platformAffected(t.state().core.tx.ExecContext(ctx, `UPDATE platform_model.configurations SET revoked=true WHERE workspace_id=$1 AND room_id=$2 AND game_id=$3 AND seat_id=$4 AND selection=$5 AND version=$6 AND NOT revoked`, sc.WorkspaceID, sc.RoomID, sc.GameID, seat, id, version)))
}

var _ model.Storage = (*PlatformModelStorage)(nil)
