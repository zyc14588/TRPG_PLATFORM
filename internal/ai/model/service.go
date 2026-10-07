// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package model

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	store "github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"io"
	"slices"
	"sync"
)

type Options struct {
	Authority       *auth.RoomAuthority
	Rooms           room.Storage
	Launches        launch.Storage
	Storage         Storage
	Vault           *credential.Vault
	Endpoints       []Endpoint
	Certifications  []Certification
	Defaults        map[string]string
	WorkspaceLimits map[string]Limits
}
type Service struct{ data **serviceData }
type serviceData struct {
	authority    *auth.RoomAuthority
	rooms        room.Storage
	launches     launch.Storage
	storage      Storage
	vault        *credential.Vault
	endpoints    map[string]EndpointData
	certificates map[string]CertificationData
	defaults     map[string]string
	caps         map[string]Limits
	mu           sync.RWMutex
	revoked      map[string]bool
}

func (*Service) Format(s fmt.State, _ rune)   { _, _ = io.WriteString(s, "<private model service>") }
func (*Service) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func New(o Options) (*Service, error) {
	if o.Authority == nil || o.Rooms == nil || o.Launches == nil || o.Storage == nil || o.Vault == nil || len(o.Endpoints) > 128 || len(o.Certifications) > 128 || len(o.Defaults) > 128 || len(o.WorkspaceLimits) > 128 {
		return nil, auth.ErrInvalid
	}
	d := &serviceData{authority: o.Authority, rooms: o.Rooms, launches: o.Launches, storage: o.Storage, vault: o.Vault, endpoints: map[string]EndpointData{}, certificates: map[string]CertificationData{}, defaults: map[string]string{}, caps: map[string]Limits{}, revoked: map[string]bool{}}
	for _, v := range o.Endpoints {
		e := NewEndpoint(v.StorageValue()).StorageValue()
		if !canonicalEndpoint(e) {
			return nil, auth.ErrInvalid
		}
		if _, ok := d.endpoints[e.ID]; ok {
			return nil, auth.ErrInvalid
		}
		d.endpoints[e.ID] = e
	}
	for _, v := range o.Certifications {
		c := NewCertification(v.StorageValue()).StorageValue()
		if !validCertificate(c, d.endpoints) {
			return nil, auth.ErrInvalid
		}
		key := c.WorkspaceID + "/" + c.ID
		if _, ok := d.certificates[key]; ok {
			return nil, auth.ErrInvalid
		}
		d.certificates[key] = c
	}
	for w, c := range o.WorkspaceLimits {
		if !store.ValidID(w) || !validBudget(c) {
			return nil, auth.ErrInvalid
		}
		d.caps[w] = c
	}
	for w, id := range o.Defaults {
		if _, ok := d.certificates[w+"/"+id]; !ok {
			return nil, auth.ErrInvalid
		}
		if _, ok := d.caps[w]; !ok {
			return nil, auth.ErrInvalid
		}
		d.defaults[w] = id
	}
	return &Service{data: &d}, nil
}

// RevokeCertification is a trusted operator composition seam. It issues no
// credential, identity or qualification and can only remove an existing grant.
func (s *Service) RevokeCertification(workspace, id string) error {
	d := s.state()
	if d == nil {
		return auth.ErrUnavailable
	}
	key := workspace + "/" + id
	d.mu.Lock()
	defer d.mu.Unlock()
	if _, ok := d.certificates[key]; !ok {
		return auth.ErrDenied
	}
	d.revoked[key] = true
	return nil
}
func (s *Service) certificate(workspace string, b CertificateBinding) (CertificationData, error) {
	d := s.state()
	if d == nil {
		return CertificationData{}, auth.ErrUnavailable
	}
	d.mu.RLock()
	defer d.mu.RUnlock()
	key := workspace + "/" + b.ID
	c, ok := d.certificates[key]
	if !ok || d.revoked[key] || b.Hash != "" && b.Hash != digest(c) {
		return CertificationData{}, auth.ErrDenied
	}
	return c, nil
}
func (s *Service) do(ctx context.Context, c Caller, action string, target TargetData, canonical any, calls auth.RoomCallbacks) (auth.Outcome, error) {
	d := s.state()
	if d == nil || ctx == nil || ctx.Err() != nil || !scope(target.Scope) || !store.ValidID(target.SeatID) || !store.ValidID(target.ID) {
		return auth.Outcome{}, auth.ErrInvalid
	}
	raw, e := json.Marshal(canonical)
	if e != nil || len(raw) > 16_384 {
		return auth.Outcome{}, auth.ErrInvalid
	}
	caller := c.StorageValue()
	cmd := auth.RoomSecret(auth.RoomCommandData{Action: action, TargetKey: target.Scope.WorkspaceID + "/" + target.Scope.RoomID + "/" + target.Scope.GameID + "/" + target.SeatID + "/" + target.ID, Canonical: raw, Write: true})
	out, e := d.authority.Do(ctx, caller.Credential, caller.CSRF, caller.IdempotencyKey, caller.Network, cmd, calls)
	return out, auth.SafeError(e)
}
func receipt(kind, id string, version uint64) (auth.Outcome, error) {
	raw, e := json.Marshal(struct {
		Kind, ID string
		Version  uint64
	}{kind, id, version})
	if e != nil {
		return auth.Outcome{}, auth.ErrUnavailable
	}
	return auth.RoomOutcome(raw), nil
}
func sameReceipt(out auth.Outcome, kind, id string, v uint64) bool {
	expected, e := receipt(kind, id, v)
	return e == nil && slices.Equal(expected.StorageValue().Body, out.StorageValue().Body)
}

func (s *Service) StoreCredential(ctx context.Context, caller Caller, request CredentialRequest) error {
	r := request.StorageValue()
	if r.Lifetime != credential.Temporary && r.Lifetime != credential.Retained {
		return auth.ErrInvalid
	}
	var fingerprint string
	e := r.Key.Use(func(raw []byte) error { h := sha256.Sum256(raw); fingerprint = hex.EncodeToString(h[:]); return nil })
	if e != nil {
		return e
	}
	canonical := struct {
		Scope       core.Scope
		SeatID, ID  string
		Lifetime    credential.Lifetime
		ExpiresAt   string
		Fingerprint string
	}{r.Scope, r.SeatID, r.ID, r.Lifetime, r.ExpiresAt.UTC().Format("2006-01-02T15:04:05.999999999Z07:00"), fingerprint}
	work := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved *auth.Outcome) (auth.Outcome, error) {
		f, e := s.frame(ctx, tx.Core(), r.Scope, r.SeatID)
		if e != nil {
			return auth.Outcome{}, e
		}
		part, e := f.access(ctx, tx.Core(), v)
		if e != nil {
			return auth.Outcome{}, e
		}
		kind, id := identity(v)
		if kind == "" {
			return auth.Outcome{}, auth.ErrDenied
		}
		if r.Lifetime == credential.Retained {
			if e = retained(ctx, tx.Core(), r.Scope, kind, id); e != nil {
				return auth.Outcome{}, e
			}
		} else if r.ExpiresAt.IsZero() || r.ExpiresAt.After(v.ExpiresAt) {
			return auth.Outcome{}, auth.ErrDenied
		}
		_ = part
		if saved != nil {
			record, e := f.mt.Credential(ctx, r.Scope, r.SeatID, r.ID)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			b := record.StorageValue().Binding
			if b.OwnerKind != kind || b.OwnerID != id || b.Lifetime != r.Lifetime || !b.ExpiresAt.Equal(r.ExpiresAt) {
				return auth.Outcome{}, auth.ErrConflict
			}
			if e = s.credentialCurrent(ctx, tx.Core(), f, record, kind, id); e != nil {
				return auth.Outcome{}, e
			}
			if !sameReceipt(*saved, "credential", r.ID, b.Version) {
				return auth.Outcome{}, auth.ErrConflict
			}
			return *saved, nil
		}
		now, e := tx.Core().Now(ctx)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		binding := credential.Binding{Scope: r.Scope, SeatID: r.SeatID, ID: r.ID, OwnerKind: kind, OwnerID: id, Lifetime: r.Lifetime, ExpiresAt: r.ExpiresAt, Version: 1}
		record, e := s.state().vault.Seal(ctx, binding, r.Key, now)
		if e != nil {
			return auth.Outcome{}, e
		}
		if e = f.mt.InsertCredential(ctx, record); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		return receipt("credential", r.ID, 1)
	}
	_, e = s.do(ctx, caller, "model.credential", TargetData{r.Scope, r.SeatID, r.ID}, canonical, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		return work(c, t, v, nil)
	}, Replay: func(c context.Context, t auth.Transaction, v auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, v, &o)
		return e
	}})
	return e
}
func (s *Service) Configure(ctx context.Context, caller Caller, request ConfigureRequest) (Configuration, error) {
	r := NewConfigureRequest(request.StorageValue()).StorageValue()
	if !validBudget(r.Budget) || !labels(r.FallbackIDs, 4) || !store.ValidID(r.CredentialID) || r.ExpectedVersion >= 1<<53 {
		return Configuration{}, auth.ErrInvalid
	}
	if r.CertificationID == "" && s.state() != nil {
		r.CertificationID = s.state().defaults[r.Scope.WorkspaceID]
	}
	if !store.ValidID(r.CertificationID) {
		return Configuration{}, auth.ErrDenied
	}
	var out Configuration
	work := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved *auth.Outcome) (auth.Outcome, error) {
		f, e := s.frame(ctx, tx.Core(), r.Scope, r.SeatID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if f.slot.Mode != "ai" || f.slot.ModelSelection != r.Selection {
			return auth.Outcome{}, auth.ErrDenied
		}
		if _, e = f.access(ctx, tx.Core(), v); e != nil {
			return auth.Outcome{}, e
		}
		kind, id := identity(v)
		cap, ok := s.state().caps[r.Scope.WorkspaceID]
		if !ok || !fits(r.Budget, cap) {
			return auth.Outcome{}, auth.ErrDenied
		}
		now, e := tx.Core().Now(ctx)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		cert, e := s.certificate(r.Scope.WorkspaceID, CertificateBinding{ID: r.CertificationID})
		if e != nil || cert.Level < 2 || !certified(cert, f.prep.GraphHash, nil, now) {
			return auth.Outcome{}, auth.ErrDenied
		}
		if cert.Tuple.ToolMode == "structured" && r.Budget.Tools < 1 {
			return auth.Outcome{}, auth.ErrDenied
		}
		fallbacks := []CertificateBinding{}
		for _, id := range r.FallbackIDs {
			if id == cert.ID {
				return auth.Outcome{}, auth.ErrInvalid
			}
			c, e := s.certificate(r.Scope.WorkspaceID, CertificateBinding{ID: id})
			if e != nil || c.Level < cert.Level || !certified(c, f.prep.GraphHash, cert.Capabilities, now) || c.Tuple.ToolMode != cert.Tuple.ToolMode || c.Tuple.PromptTemplate != cert.Tuple.PromptTemplate {
				return auth.Outcome{}, auth.ErrDenied
			}
			fallbacks = append(fallbacks, CertificateBinding{ID: id, Hash: digest(c)})
		}
		record, e := f.mt.Credential(ctx, r.Scope, r.SeatID, r.CredentialID)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		if e = s.credentialCurrent(ctx, tx.Core(), f, record, kind, id); e != nil {
			return auth.Outcome{}, e
		}
		value := ConfigurationData{Scope: r.Scope, SeatID: r.SeatID, Selection: r.Selection, OwnerKind: kind, OwnerID: id, ConfigurationID: f.prep.ConfigurationID, ConfigurationHash: f.prep.ConfigurationHash, GraphHash: f.prep.GraphHash, PreparationRevision: f.prep.Revision, Version: r.ExpectedVersion + 1, CredentialID: r.CredentialID, CredentialVersion: record.StorageValue().Binding.Version, Primary: CertificateBinding{ID: cert.ID, Hash: digest(cert)}, Tuple: cert.Tuple, Fallbacks: fallbacks, Budget: r.Budget}
		if saved != nil {
			stored, e := f.mt.Configuration(ctx, r.Scope, r.SeatID, r.Selection)
			if e != nil {
				return auth.Outcome{}, auth.SafeError(e)
			}
			if digest(stored.StorageValue()) != digest(value) || !sameReceipt(*saved, "model", r.Selection, value.Version) {
				return auth.Outcome{}, auth.ErrConflict
			}
			out = CopyConfiguration(stored)
			return *saved, nil
		}
		out = CopyConfiguration(auth.RoomSecret(value))
		if e = f.mt.PutConfiguration(ctx, out, r.ExpectedVersion); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		return receipt("model", r.Selection, value.Version)
	}
	_, e := s.do(ctx, caller, "model.configure", TargetData{r.Scope, r.SeatID, r.Selection}, r, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		return work(c, t, v, nil)
	}, Replay: func(c context.Context, t auth.Transaction, v auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, v, &o)
		return e
	}})
	if e != nil {
		return Configuration{}, e
	}
	return out, nil
}
func (s *Service) RevokeCredential(ctx context.Context, caller Caller, target Target) error {
	r := target.StorageValue()
	work := func(ctx context.Context, tx auth.Transaction, v auth.SessionData, saved *auth.Outcome) (auth.Outcome, error) {
		f, e := s.frame(ctx, tx.Core(), r.Scope, r.SeatID)
		if e != nil {
			return auth.Outcome{}, e
		}
		if _, e = f.access(ctx, tx.Core(), v); e != nil {
			return auth.Outcome{}, e
		}
		record, e := f.mt.Credential(ctx, r.Scope, r.SeatID, r.ID)
		if e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		b := record.StorageValue().Binding
		kind, id := identity(v)
		if b.OwnerKind != kind || b.OwnerID != id {
			return auth.Outcome{}, auth.ErrDenied
		}
		if saved != nil {
			if !record.StorageValue().Revoked || !sameReceipt(*saved, "revoked", r.ID, b.Version) {
				return auth.Outcome{}, auth.ErrConflict
			}
			return *saved, nil
		}
		if e = f.mt.RevokeCredential(ctx, r.Scope, r.SeatID, r.ID, b.Version); e != nil {
			return auth.Outcome{}, auth.SafeError(e)
		}
		return receipt("revoked", r.ID, b.Version)
	}
	_, e := s.do(ctx, caller, "model.revoke-key", r, r, auth.RoomCallbacks{Apply: func(c context.Context, t auth.Transaction, v auth.SessionData) (auth.Outcome, error) {
		return work(c, t, v, nil)
	}, Replay: func(c context.Context, t auth.Transaction, v auth.SessionData, o auth.Outcome) error {
		_, e := work(c, t, v, &o)
		return e
	}})
	return e
}

// Check runs only inside the existing authenticated launch transaction. It
// reads current rows and qualifications; there is no provider/network work.
func (s *Service) Check(ctx context.Context, tx core.Transaction, proof launch.ModelRequirement, acks []launch.Acknowledgment) error {
	r := proof.StorageValue()
	if s.state() == nil {
		return auth.ErrDenied
	}
	v, e := auth.RoomAdmissionSession(tx)
	if e != nil {
		return auth.ErrDenied
	}
	f, e := s.frame(ctx, tx, r.Scope, r.SeatID)
	if e != nil {
		return e
	}
	if _, e = f.access(ctx, tx, v.StorageValue()); e != nil {
		return e
	}
	if f.slot.Mode != "ai" || f.slot.ModelSelection != r.Selection || r.ConfigurationID != f.prep.ConfigurationID || r.ConfigurationHash != f.prep.ConfigurationHash || r.GraphHash != f.prep.GraphHash || r.Revision != f.prep.Revision || !labels(r.Capabilities, 32) {
		return auth.ErrDenied
	}
	config, e := f.mt.Configuration(ctx, r.Scope, r.SeatID, r.Selection)
	if e != nil {
		return auth.SafeError(e)
	}
	c := config.StorageValue()
	if c.Revoked || c.ConfigurationID != r.ConfigurationID || c.ConfigurationHash != r.ConfigurationHash || c.GraphHash != r.GraphHash || c.PreparationRevision != r.Revision {
		return auth.ErrDenied
	}
	// Readiness consent is checked by launch. Bind its current acknowledgments
	// again here so a stored model cannot bypass the same safety revision.
	if len(acks) < 1 || len(acks) > 64 {
		return auth.ErrDenied
	}
	actual, e := f.lt.Acknowledgments(ctx, r.Scope)
	if e != nil || len(actual) != len(acks) {
		return auth.ErrDenied
	}
	current := map[string]string{}
	for _, stored := range actual {
		a := stored.StorageValue()
		current[a.ParticipantID] = digest(a)
	}
	seen := map[string]bool{}
	for _, ack := range acks {
		a := ack.StorageValue()
		if a.Scope != r.Scope || a.ConfigurationHash != r.ConfigurationHash || a.GraphHash != r.GraphHash || a.Revision != r.Revision || !a.Consent || !a.Ready || !a.SafetyConfirmed || seen[a.ParticipantID] || current[a.ParticipantID] != digest(a) {
			return auth.ErrDenied
		}
		seen[a.ParticipantID] = true
	}
	return s.configurationCurrent(ctx, tx, f, c, r.Capabilities)
}
func (s *Service) Read(ctx context.Context, caller Caller, target Target) (Configuration, error) {
	d := s.state()
	if d == nil {
		return Configuration{}, auth.ErrUnavailable
	}
	r := target.StorageValue()
	var out Configuration
	c := caller.StorageValue()
	e := d.authority.Inspect(ctx, c.Credential, c.CSRF, false, func(ctx context.Context, tx auth.Transaction, v auth.SessionData) error {
		f, e := s.frame(ctx, tx.Core(), r.Scope, r.SeatID)
		if e != nil {
			return e
		}
		if _, e = f.access(ctx, tx.Core(), v); e != nil {
			return e
		}
		stored, e := f.mt.Configuration(ctx, r.Scope, r.SeatID, r.ID)
		if e != nil {
			return auth.SafeError(e)
		}
		if e = s.configurationCurrent(ctx, tx.Core(), f, stored.StorageValue(), nil); e != nil {
			return e
		}
		out = CopyConfiguration(stored)
		return nil
	})
	if e != nil {
		return Configuration{}, auth.SafeError(e)
	}
	return out, nil
}

var _ launch.ModelChecker = (*Service)(nil)
