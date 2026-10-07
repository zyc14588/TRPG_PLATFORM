// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"context"
	"crypto/aes"
	"crypto/cipher"
	"crypto/hmac"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"sync"
	"time"

	"github.com/santhosh-tekuri/jsonschema/v6"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

type Service struct{ data **serviceData }
type rateEntry struct {
	start time.Time
	count int
}
type serviceData struct {
	repo      Repository
	verifier  AdmissionVerifier
	cookieKey []byte
	replay    cipher.AEAD
	schemas   map[string]*jsonschema.Schema
	hashing   chan struct{}
	mu        sync.Mutex
	rates     map[string]rateEntry
	global    rateEntry
}

func NewService(repo Repository, cookieKey, replayKey, schema []byte, verifier AdmissionVerifier) (*Service, error) {
	if repo == nil || len(cookieKey) != 32 || len(replayKey) != 32 || hmac.Equal(cookieKey, replayKey) {
		return nil, ErrInvalid
	}
	schemas, e := compileContract(schema)
	if e != nil {
		return nil, e
	}
	block, e := aes.NewCipher(replayKey)
	if e != nil {
		return nil, ErrUnavailable
	}
	replay, e := cipher.NewGCM(block)
	if e != nil {
		return nil, ErrUnavailable
	}
	d := &serviceData{repo: repo, verifier: verifier, cookieKey: append([]byte(nil), cookieKey...), replay: replay, schemas: schemas, hashing: make(chan struct{}, 2), rates: make(map[string]rateEntry)}
	return &Service{data: &d}, nil
}
func (*Service) String() string             { return "<authentication service>" }
func (*Service) GoString() string           { return "<authentication service>" }
func (*Service) Format(f fmt.State, _ rune) { _, _ = io.WriteString(f, "<authentication service>") }
func (s *Service) state() *serviceData {
	if s == nil || s.data == nil {
		return nil
	}
	return *s.data
}
func (s *Service) transact(ctx context.Context, f func(Transaction) error) error {
	d := s.state()
	if d == nil || ctx == nil || ctx.Err() != nil {
		return ErrUnavailable
	}
	return SafeError(d.repo.Transact(ctx, f))
}
func (s *Service) limit(key string) error {
	d := s.state()
	if d == nil {
		return ErrUnavailable
	}
	now := time.Now()
	d.mu.Lock()
	defer d.mu.Unlock()
	for k, v := range d.rates {
		if now.Sub(v.start) >= time.Minute {
			delete(d.rates, k)
		}
	}
	if now.Sub(d.global.start) >= time.Minute {
		d.global = rateEntry{start: now}
	}
	if d.global.count >= 60 {
		return ErrRateLimited
	}
	v, ok := d.rates[key]
	if !ok {
		if len(d.rates) >= 256 {
			return ErrRateLimited
		}
		v.start = now
	}
	if v.count >= 10 {
		return ErrRateLimited
	}
	v.count++
	d.rates[key] = v
	d.global.count++
	return nil
}
func token() (string, error) {
	b := make([]byte, 32)
	if _, e := rand.Read(b); e != nil {
		return "", ErrUnavailable
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}
func tokenHash(raw string) string {
	h := sha256.Sum256([]byte(raw))
	return base64.RawURLEncoding.EncodeToString(h[:])
}
func (s *Service) mac(text string) string {
	h := hmac.New(sha256.New, s.state().cookieKey)
	_, _ = io.WriteString(h, text)
	return base64.RawURLEncoding.EncodeToString(h.Sum(nil))
}
func (s *Service) csrf(v SessionData) string {
	return s.mac("csrf-v1\x00" + v.Hash + "\x00" + v.ExpiresAt.UTC().Format(time.RFC3339))
}
func sessionTime(now time.Time) time.Time { return now.UTC().Truncate(time.Microsecond) }

func (s *Service) newSession(ctx context.Context, tx Transaction, kind, account string, guest core.Guest) (Session, BrowserCredential, error) {
	now, e := tx.Core().Now(ctx)
	if e != nil {
		return Session{}, BrowserCredential{}, SafeError(e)
	}
	now = sessionTime(now)
	raw, e := token()
	if e != nil {
		return Session{}, BrowserCredential{}, e
	}
	v := SessionData{Hash: tokenHash(raw), Kind: kind, AccountID: account, GuestID: guest.ID, Scope: guest.Scope, ExpiresAt: now.Add(8 * time.Hour), LastSeen: now}
	if kind == "preauth" {
		v.ExpiresAt = now.Add(5 * time.Minute)
	}
	if kind == "guest" && guest.ExpiresAt.Before(v.ExpiresAt) {
		v.ExpiresAt = sessionTime(guest.ExpiresAt)
	}
	if !v.ExpiresAt.After(now) {
		return Session{}, BrowserCredential{}, ErrDenied
	}
	ss := StoredSession(v)
	if e := tx.PutSession(ctx, ss); e != nil {
		return Session{}, BrowserCredential{}, SafeError(e)
	}
	return ss, BrowserCookie(raw), nil
}

func (s *Service) live(ctx context.Context, tx Transaction, v SessionData, retired bool, claimed string) error {
	now, e := tx.Core().Now(ctx)
	if e != nil {
		return SafeError(e)
	}
	if v.Revoked || v.Retired && !retired || !now.Before(v.ExpiresAt) || !now.Before(v.LastSeen.Add(30*time.Minute)) {
		return ErrUnauthenticated
	}
	switch v.Kind {
	case "preauth":
		return nil
	case "account":
		a, e := tx.Core().Account(ctx, v.AccountID)
		if e != nil {
			if e == core.ErrDenied {
				return ErrUnauthenticated
			}
			return SafeError(e)
		}
		if a.Disabled {
			return ErrUnauthenticated
		}
	case "guest":
		if _, e := tx.Core().Workspace(ctx, v.Scope.WorkspaceID); e != nil {
			return SafeError(e)
		}
		g, e := tx.Core().Guest(ctx, v.Scope, v.GuestID)
		if e != nil {
			return SafeError(e)
		}
		if g.Disabled || !now.Before(g.ExpiresAt) || g.ClaimedAccountID != claimed {
			return ErrUnauthenticated
		}
	default:
		return ErrUnauthenticated
	}
	return nil
}
func (s *Service) findSession(ctx context.Context, tx Transaction, credential BrowserCredential) (SessionData, error) {
	raw := credential.StorageValue()
	if !tokenID.MatchString(raw) {
		return SessionData{}, ErrUnauthenticated
	}
	v, e := tx.Session(ctx, tokenHash(raw))
	if e == ErrDenied || e == core.ErrDenied {
		return SessionData{}, ErrUnauthenticated
	}
	return v.StorageValue(), SafeError(e)
}
func contextData(s *Service, ctx context.Context, tx Transaction, v SessionData) (map[string]any, error) {
	data := map[string]any{"csrf_token": s.csrf(v)}
	if v.Kind == "preauth" {
		data["state"] = "anonymous"
		data["csrf_expires_at"] = v.ExpiresAt.Format(time.RFC3339Nano)
		return data, nil
	}
	data["state"] = "authenticated"
	data["expires_at"] = v.ExpiresAt.Format(time.RFC3339Nano)
	if v.Kind == "account" {
		a, e := tx.Core().Account(ctx, v.AccountID)
		if e != nil {
			return nil, SafeError(e)
		}
		data["principal"] = map[string]any{"kind": "account", "account_id": a.ID, "display_name": a.DisplayName}
	} else {
		data["principal"] = map[string]any{"kind": "guest", "participation_id": v.GuestID, "scope": scopeData(v.Scope)}
	}
	return data, nil
}
func scopeData(v core.Scope) map[string]any {
	return map[string]any{"workspace_id": v.WorkspaceID, "room_id": v.RoomID, "game_id": v.GameID}
}
func workspaceData(v core.Workspace) map[string]any {
	return map[string]any{"workspace_id": v.ID, "name": v.Name, "owner_account_id": v.OwnerID}
}
func (s *Service) response(action string, data any, cookie BrowserCredential, kind string, expiry time.Time) (Outcome, error) {
	id, e := token()
	if e != nil {
		return Outcome{}, e
	}
	value := map[string]any{"schema_version": 1, "request_id": "r_" + id, "data": data}
	bytes, e := json.Marshal(value)
	if e != nil {
		return Outcome{}, ErrUnavailable
	}
	decoded, e := strictJSON(bytes)
	if e != nil {
		return Outcome{}, ErrUnavailable
	}
	if e := s.state().schemas[responseSchemas[action]].Validate(decoded); e != nil {
		return Outcome{}, ErrUnavailable
	}
	return protect(OutcomeData{Body: bytes, Cookie: cookie, CookieKind: kind, ExpiresAt: expiry}), nil
}
func (s *Service) Context(ctx context.Context, credential BrowserCredential, network string) (Outcome, error) {
	var out Outcome
	if credential.StorageValue() == "" {
		if e := s.limit(s.mac("context\x00" + network)); e != nil {
			return out, e
		}
	}
	e := s.transact(ctx, func(tx Transaction) error {
		var v SessionData
		var cookie BrowserCredential
		var e error
		if credential.StorageValue() == "" {
			ss, c, e := s.newSession(ctx, tx, "preauth", "", core.Guest{})
			if e != nil {
				return e
			}
			v = ss.StorageValue()
			cookie = c
		} else {
			v, e = s.findSession(ctx, tx, credential)
			if e != nil {
				return e
			}
			if e = s.live(ctx, tx, v, false, ""); e != nil {
				return e
			}
			now, e := tx.Core().Now(ctx)
			if e != nil {
				return SafeError(e)
			}
			v.LastSeen = sessionTime(now)
			if e = tx.PutSession(ctx, StoredSession(v)); e != nil {
				return SafeError(e)
			}
		}
		data, e := contextData(s, ctx, tx, v)
		if e != nil {
			return e
		}
		out, e = s.response("context", data, cookie, v.Kind, v.ExpiresAt)
		return e
	})
	if e != nil {
		return Outcome{}, e
	}
	return out, nil
}
func (s *Service) Ready(ctx context.Context) bool {
	return s.transact(ctx, func(tx Transaction) error { _, e := tx.Core().Now(ctx); return e }) == nil
}

func actor(ctx context.Context, tx Transaction, v SessionData) (*core.Service, core.Actor, error) {
	c, e := core.NewService(singleTransaction{tx: tx.Core()})
	if e != nil {
		return nil, core.Actor{}, SafeError(e)
	}
	if v.Kind != "account" {
		return nil, core.Actor{}, ErrDenied
	}
	a, e := c.AccountActor(ctx, v.AccountID)
	return c, a, SafeError(e)
}
func (s *Service) Workspace(ctx context.Context, credential BrowserCredential, id string) (Outcome, error) {
	if !wireID.MatchString(id) {
		return Outcome{}, ErrInvalid
	}
	var out Outcome
	e := s.transact(ctx, func(tx Transaction) error {
		v, e := s.findSession(ctx, tx, credential)
		if e != nil {
			return e
		}
		if e = s.live(ctx, tx, v, false, ""); e != nil {
			return e
		}
		c, a, e := actor(ctx, tx, v)
		if e != nil {
			return e
		}
		w, e := c.Workspace(ctx, a, id)
		if e != nil {
			return SafeError(e)
		}
		out, e = s.response("workspace", workspaceData(w), BrowserCredential{}, "", time.Time{})
		return e
	})
	if e != nil {
		return Outcome{}, e
	}
	return out, nil
}
func field(r RequestData, name string) string { v, _ := r.Fields[name].(string); return v }
func (s *Service) credential(ctx context.Context, tx Transaction, login, password string) (string, error) {
	id, stored, e := tx.Credential(ctx, login)
	if e != nil && e != ErrDenied && e != core.ErrDenied {
		return "", SafeError(e)
	}
	ok := verifyPassword(password, stored)
	if e != nil || !ok {
		return "", ErrUnauthenticated
	}
	a, e := tx.Core().Account(ctx, id)
	if e != nil {
		if e == core.ErrDenied {
			return "", ErrUnauthenticated
		}
		return "", SafeError(e)
	}
	if a.Disabled {
		return "", ErrUnauthenticated
	}
	return id, nil
}
func createAccount(ctx context.Context, tx Transaction, r RequestData) (string, error) {
	id, e := token()
	if e != nil {
		return "", e
	}
	id = "a_" + id
	p, e := newPassword(field(r, "password"))
	if e != nil {
		return "", e
	}
	c, e := core.NewService(singleTransaction{tx: tx.Core()})
	if e != nil {
		return "", SafeError(e)
	}
	if e := c.RegisterAccount(ctx, id, field(r, "display_name")); e != nil {
		return "", SafeError(e)
	}
	if e := tx.PutCredential(ctx, field(r, "login_name"), id, p); e != nil {
		return "", SafeError(e)
	}
	return id, nil
}

type encryptedOutcome struct {
	Body         []byte
	Cookie, Kind string
	ExpiresAt    time.Time
}

func receiptAAD(v ReceiptData) string {
	return v.OwnerHash + "\x00" + v.Endpoint + "\x00" + v.Key + "\x00" + v.RequestDigest
}
func (s *Service) sealReceipt(v ReceiptData, out Outcome) (Receipt, error) {
	o := out.StorageValue()
	plaintext, e := json.Marshal(encryptedOutcome{o.Body, o.Cookie.StorageValue(), o.CookieKind, o.ExpiresAt})
	if e != nil {
		return Receipt{}, ErrUnavailable
	}
	defer clear(plaintext)
	aead := s.state().replay
	nonce := make([]byte, aead.NonceSize())
	if _, e := rand.Read(nonce); e != nil {
		return Receipt{}, ErrUnavailable
	}
	v.Ciphertext = aead.Seal(nonce, nonce, plaintext, []byte(receiptAAD(v)))
	return StoredReceipt(v), nil
}
func (s *Service) openReceipt(v ReceiptData) (Outcome, error) {
	aead := s.state().replay
	if len(v.Ciphertext) < aead.NonceSize()+aead.Overhead() {
		return Outcome{}, ErrUnavailable
	}
	plain, e := aead.Open(nil, v.Ciphertext[:aead.NonceSize()], v.Ciphertext[aead.NonceSize():], []byte(receiptAAD(v)))
	if e != nil {
		return Outcome{}, ErrUnavailable
	}
	defer clear(plain)
	var vout encryptedOutcome
	if e := json.Unmarshal(plain, &vout); e != nil {
		return Outcome{}, ErrUnavailable
	}
	return protect(OutcomeData{Body: vout.Body, Cookie: BrowserCookie(vout.Cookie), CookieKind: vout.Kind, ExpiresAt: vout.ExpiresAt}), nil
}
func endpoint(r RequestData) string { return r.Action + "/" + r.WorkspaceID + "/" + r.AccountID }

func (s *Service) replayAuthorized(ctx context.Context, tx Transaction, original SessionData, r RequestData, out Outcome) error {
	o := out.StorageValue()
	var successor SessionData
	claimed := ""
	if o.Cookie.StorageValue() != "" {
		var e error
		successor, e = s.findSession(ctx, tx, o.Cookie)
		if e != nil {
			return e
		}
		if original.Retired && original.SuccessorHash != successor.Hash {
			return ErrUnauthenticated
		}
		if e := s.live(ctx, tx, successor, false, ""); e != nil {
			return e
		}
		if r.Action == "claim" {
			claimed = successor.AccountID
		}
	}
	if e := s.live(ctx, tx, original, true, claimed); e != nil {
		return e
	}
	if original.Retired && o.Cookie.StorageValue() == "" {
		return ErrUnauthenticated
	}
	if r.Action == "create_workspace" {
		var body struct {
			Data struct {
				ID string `json:"workspace_id"`
			}
		}
		if e := json.Unmarshal(o.Body, &body); e != nil {
			return ErrUnavailable
		}
		c, a, e := actor(ctx, tx, original)
		if e != nil {
			return e
		}
		_, e = c.Workspace(ctx, a, body.Data.ID)
		return SafeError(e)
	}
	if r.Action == "set_member" || r.Action == "remove_member" {
		c, a, e := actor(ctx, tx, original)
		if e != nil {
			return e
		}
		if e := c.Authorize(ctx, a, core.Scope{WorkspaceID: r.WorkspaceID}, core.ManageWorkspace); e != nil {
			return SafeError(e)
		}
		w, e := tx.Core().Workspace(ctx, r.WorkspaceID)
		if e != nil {
			return SafeError(e)
		}
		if w.OwnerID == r.AccountID {
			return ErrDenied
		}
		m, e := tx.Core().Membership(ctx, r.WorkspaceID, original.AccountID)
		if e != nil {
			return SafeError(e)
		}
		if m.Role != core.Owner {
			current, e := tx.Core().Membership(ctx, r.WorkspaceID, r.AccountID)
			if e != nil && e != core.ErrDenied {
				return SafeError(e)
			}
			if field(r, "role") == "admin" || current.Role == core.Admin || current.Role == core.Owner {
				return ErrDenied
			}
		}
	}
	return nil
}

func (s *Service) Mutate(ctx context.Context, credential BrowserCredential, csrf, key, network string, request Request) (Outcome, error) {
	d := s.state()
	if d == nil {
		return Outcome{}, ErrUnavailable
	}
	r := request.StorageValue()
	if requestSchemas[r.Action] == "" || !idemID.MatchString(key) || !tokenID.MatchString(csrf) {
		return Outcome{}, ErrInvalid
	}
	if e := d.schemas[requestSchemas[r.Action]].Validate(r.Fields); e != nil {
		return Outcome{}, ErrInvalid
	}
	if r.WorkspaceID != "" && !wireID.MatchString(r.WorkspaceID) || r.AccountID != "" && !wireID.MatchString(r.AccountID) {
		return Outcome{}, ErrInvalid
	}
	if e := s.limit(s.mac(network + "\x00" + r.Action + "\x00" + field(r, "login_name"))); e != nil {
		return Outcome{}, e
	}
	if r.Action == "login" || r.Action == "register" || r.Action == "claim" {
		select {
		case d.hashing <- struct{}{}:
			defer func() { <-d.hashing }()
		default:
			return Outcome{}, ErrRateLimited
		}
	}
	canonical, e := json.Marshal(r)
	if e != nil {
		return Outcome{}, ErrInvalid
	}
	digest := s.mac("request-v1\x00" + string(canonical))
	clear(canonical)
	var out Outcome
	e = s.transact(ctx, func(tx Transaction) error {
		v, e := s.findSession(ctx, tx, credential)
		if e != nil {
			return e
		}
		if !hmac.Equal([]byte(csrf), []byte(s.csrf(v))) {
			return ErrDenied
		}
		now, e := tx.Core().Now(ctx)
		if e != nil {
			return SafeError(e)
		}
		now = sessionTime(now)
		receipt, e := tx.Receipt(ctx, v.Hash, endpoint(r), key)
		if e == nil {
			saved := receipt.StorageValue()
			if saved.RequestDigest != digest {
				return ErrConflict
			}
			if !now.Before(saved.ExpiresAt) {
				return ErrConflict
			}
			out, e = s.openReceipt(saved)
			if e != nil {
				return e
			}
			return s.replayAuthorized(ctx, tx, v, r, out)
		}
		if e != ErrDenied && e != core.ErrDenied {
			return SafeError(e)
		}
		if e := s.live(ctx, tx, v, false, ""); e != nil {
			return e
		}
		v.LastSeen = now
		if e := tx.PutSession(ctx, StoredSession(v)); e != nil {
			return SafeError(e)
		}
		out, e = s.apply(ctx, tx, v, r)
		if e != nil {
			return e
		}
		expires := now.Add(5 * time.Minute)
		if v.ExpiresAt.Before(expires) {
			expires = v.ExpiresAt
		}
		saved, e := s.sealReceipt(ReceiptData{OwnerHash: v.Hash, Endpoint: endpoint(r), Key: key, RequestDigest: digest, ExpiresAt: expires}, out)
		if e != nil {
			return e
		}
		return SafeError(tx.PutReceipt(ctx, saved))
	})
	if e != nil {
		return Outcome{}, e
	}
	return out, nil
}

func (s *Service) rotate(ctx context.Context, tx Transaction, old SessionData, kind, account string, guest core.Guest, action string, participation *core.Participation) (Outcome, error) {
	next, cookie, e := s.newSession(ctx, tx, kind, account, guest)
	if e != nil {
		return Outcome{}, e
	}
	v := next.StorageValue()
	old.Retired = true
	old.SuccessorHash = v.Hash
	if e := tx.PutSession(ctx, StoredSession(old)); e != nil {
		return Outcome{}, SafeError(e)
	}
	data, e := contextData(s, ctx, tx, v)
	if e != nil {
		return Outcome{}, e
	}
	var value any = data
	if participation != nil {
		value = map[string]any{"context": data, "participation": map[string]any{"participation_id": participation.GuestID, "scope": scopeData(participation.Scope)}}
	}
	return s.response(action, value, cookie, v.Kind, v.ExpiresAt)
}
func (s *Service) apply(ctx context.Context, tx Transaction, v SessionData, r RequestData) (Outcome, error) {
	switch r.Action {
	case "register":
		if v.Kind != "preauth" {
			return Outcome{}, ErrDenied
		}
		if e := tx.ConsumeGrant(ctx, tokenHash(field(r, "registration_token"))); e != nil {
			return Outcome{}, SafeError(e)
		}
		id, e := createAccount(ctx, tx, r)
		if e != nil {
			return Outcome{}, e
		}
		return s.rotate(ctx, tx, v, "account", id, core.Guest{}, r.Action, nil)
	case "login":
		if v.Kind == "guest" {
			return Outcome{}, ErrClaimRequired
		}
		id, e := s.credential(ctx, tx, field(r, "login_name"), field(r, "password"))
		if e != nil {
			return Outcome{}, e
		}
		return s.rotate(ctx, tx, v, "account", id, core.Guest{}, r.Action, nil)
	case "logout":
		return s.rotate(ctx, tx, v, "preauth", "", core.Guest{}, r.Action, nil)
	case "exchange":
		if v.Kind != "preauth" || s.state().verifier == nil {
			return Outcome{}, ErrDenied
		}
		g, e := s.state().verifier.Verify(ctx, tx.Core(), BrowserCookie(field(r, "admission_token")))
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		c, e := core.NewService(singleTransaction{tx: tx.Core()})
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		if _, e := c.GuestActor(ctx, g.Scope, g.ID); e != nil {
			return Outcome{}, SafeError(e)
		}
		actual, e := tx.Core().Guest(ctx, g.Scope, g.ID)
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		return s.rotate(ctx, tx, v, "guest", "", actual, r.Action, nil)
	case "claim":
		if v.Kind != "guest" {
			return Outcome{}, ErrDenied
		}
		var id string
		var e error
		if field(r, "mode") == "existing_account" {
			id, e = s.credential(ctx, tx, field(r, "login_name"), field(r, "password"))
		} else if field(r, "mode") == "new_account" {
			id, e = createAccount(ctx, tx, r)
		} else {
			return Outcome{}, ErrInvalid
		}
		if e != nil {
			return Outcome{}, e
		}
		c, e := core.NewService(singleTransaction{tx: tx.Core()})
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		ga, e := c.GuestActor(ctx, v.Scope, v.GuestID)
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		aa, e := c.AccountActor(ctx, id)
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		p, e := c.ClaimGuest(ctx, ga, aa)
		if e != nil {
			return Outcome{}, SafeError(e)
		}
		return s.rotate(ctx, tx, v, "account", id, core.Guest{}, r.Action, &p)
	case "create_workspace", "set_member", "remove_member":
		c, a, e := actor(ctx, tx, v)
		if e != nil {
			return Outcome{}, e
		}
		if r.Action == "create_workspace" {
			id, e := token()
			if e != nil {
				return Outcome{}, e
			}
			id = "w_" + id
			if e := c.CreateWorkspace(ctx, a, id, field(r, "name")); e != nil {
				return Outcome{}, SafeError(e)
			}
			w, e := c.Workspace(ctx, a, id)
			if e != nil {
				return Outcome{}, SafeError(e)
			}
			return s.response(r.Action, workspaceData(w), BrowserCredential{}, "", time.Time{})
		}
		if r.Action == "set_member" {
			if e := c.SetMembership(ctx, a, r.WorkspaceID, r.AccountID, core.Role(field(r, "role"))); e != nil {
				return Outcome{}, SafeError(e)
			}
			return s.response(r.Action, map[string]any{"workspace_id": r.WorkspaceID, "account_id": r.AccountID, "role": field(r, "role")}, BrowserCredential{}, "", time.Time{})
		}
		if e := c.RemoveMembership(ctx, a, r.WorkspaceID, r.AccountID); e != nil {
			return Outcome{}, SafeError(e)
		}
		return s.response(r.Action, map[string]any{"removed": true}, BrowserCredential{}, "", time.Time{})
	default:
		return Outcome{}, ErrInvalid
	}
}
