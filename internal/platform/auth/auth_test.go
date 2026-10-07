// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"golang.org/x/crypto/argon2"
)

type noStorage struct{}

func (noStorage) Transact(context.Context, func(Transaction) error) error { return ErrUnavailable }
func testService(t *testing.T) *Service {
	t.Helper()
	schema, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal("schema fixture missing")
	}
	s, e := NewService(noStorage{}, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{2}, 32), schema, nil)
	if e != nil {
		t.Fatal(e)
	}
	return s
}
func TestApprovedWireRejectsInvalidInput(t *testing.T) {
	s := testService(t)
	for _, test := range []struct {
		name, action, body string
		ok                 bool
	}{
		{"valid_login", "login", `{"schema_version":1,"login_name":"local_user","password":"password"}`, true},
		{"duplicate_version", "login", `{"schema_version":2,"schema_version":1,"login_name":"local_user","password":"password"}`, false},
		{"duplicate_password", "login", `{"schema_version":1,"login_name":"local_user","password":"wrong","password":"password"}`, false},
		{"unmatched_surrogate", "login", `{"schema_version":1,"login_name":"local_user","password":"\ud800"}`, false},
		{"valid_surrogate_pair", "login", `{"schema_version":1,"login_name":"local_user","password":"\ud83d\ude00"}`, true},
		{"unknown_role", "login", `{"schema_version":1,"login_name":"local_user","password":"password","role":"owner"}`, false},
		{"unsupported_version", "logout", `{"schema_version":2}`, false},
		{"trailing_data", "logout", `{"schema_version":1} {}`, false},
		{"null", "logout", `null`, false},
		{"unverified_tenant", "exchange", `{"schema_version":1,"admission_token":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","workspace_id":"other"}`, false},
		{"owner_promotion", "set_member", `{"schema_version":1,"role":"owner"}`, false},
		{"valid_new_claim", "claim", `{"schema_version":1,"mode":"new_account","login_name":"local_user","password":"valid-long-password","display_name":"玩家"}`, true},
		{"mixed_claim", "claim", `{"schema_version":1,"mode":"existing_account","login_name":"local_user","password":"valid-long-password","display_name":"玩家"}`, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			_, e := s.Decode(test.action, "", "", []byte(test.body))
			if (e == nil) != test.ok {
				t.Fatalf("unexpected bounded result: %v", SafeError(e))
			}
		})
	}
	if _, e := s.Decode("login", "", "", []byte{0xff}); e != ErrInvalid {
		t.Fatal("invalid UTF8 accepted")
	}
	if _, e := s.Decode("logout", "", "", bytes.Repeat([]byte{' '}, 16385)); e != ErrInvalid {
		t.Fatal("oversized body accepted")
	}
}
func TestSchemaDriftIsRejected(t *testing.T) {
	schema, e := os.ReadFile("../../../schemas/platform/platform-auth-api-v1.schema.json")
	if e != nil {
		t.Fatal(e)
	}
	var doc map[string]any
	_ = json.Unmarshal(schema, &doc)
	doc["$defs"].(map[string]any)["LoginRequest"].(map[string]any)["additionalProperties"] = true
	changed, _ := json.Marshal(doc)
	if _, e := NewService(noStorage{}, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{2}, 32), changed, nil); e != ErrInvalid {
		t.Fatal("unapproved contract alteration accepted")
	}
	if _, e := NewService(noStorage{}, bytes.Repeat([]byte{1}, 32), bytes.Repeat([]byte{1}, 32), schema, nil); e != ErrInvalid {
		t.Fatal("cookie and replay keys shared")
	}
}
func TestPasswordUsesApprovedParametersAndExactUTF8(t *testing.T) {
	value := strings.Repeat("界", 15)
	p, e := newPassword(value)
	if e != nil {
		t.Fatal(e)
	}
	raw := p.StorageValue()
	expected := argon2.IDKey([]byte(value), raw[:16], 3, 65536, 1, 32)
	if !bytes.Equal(expected, raw[16:]) {
		t.Fatal("Argon2 parameters differ")
	}
	if !verifyPassword(value, p) || verifyPassword(value+" ", p) || verifyPassword(value, Password{}) {
		t.Fatal("password verification or unknown identity differs")
	}
	p2, e := newPassword(value)
	if e != nil || bytes.Equal(p2.StorageValue(), raw) {
		t.Fatal("random salts not independent")
	}
	for _, bad := range []string{"short", strings.Repeat("a", 129), string([]byte{0xff})} {
		if _, e := newPassword(bad); e != ErrInvalid {
			t.Fatal("invalid new password accepted")
		}
	}
	spaced, e := newPassword("  valid password  ")
	if e != nil || !verifyPassword("  valid password  ", spaced) || verifyPassword("valid password", spaced) {
		t.Fatal("password whitespace normalized")
	}
}
func TestOpaqueAuthenticationDoesNotFormatOrExportPrivateData(t *testing.T) {
	s := testService(t)
	marker := "PRIVATE_AUTH_SENTINEL_" + strings.Repeat("q", 24)
	items := []any{BrowserCookie(marker), protect([]byte(marker)), StoredSession(SessionData{Hash: marker, GuestID: marker}), StoredReceipt(ReceiptData{Key: marker, Ciphertext: []byte(marker)}), protect(RequestData{Fields: map[string]any{"password": marker}}), protect(OutcomeData{Body: []byte(marker), Cookie: BrowserCookie(marker)}), s}
	for _, item := range items {
		for _, format := range []string{"%v", "%+v", "%#v", "%s", "%q", "%d", "%f", "%x", "%T", "%w", "%*v"} {
			printed := fmt.Sprintf(format, item)
			if strings.Contains(printed, marker) {
				t.Fatal("private marker escaped formatting")
			}
		}
		if _, e := json.Marshal(item); e == nil {
			if _, ok := item.(*Service); !ok {
				t.Fatal("secret JSON export permitted")
			}
		}
	}
}
func TestReceiptAuthenticatedEncryption(t *testing.T) {
	s := testService(t)
	v := ReceiptData{OwnerHash: "owner", Endpoint: "login//", Key: "key", RequestDigest: "digest"}
	out := protect(OutcomeData{Body: []byte("private participant data"), Cookie: BrowserCookie("private-cookie"), CookieKind: "account", ExpiresAt: time.Now()})
	r, e := s.sealReceipt(v, out)
	if e != nil {
		t.Fatal(e)
	}
	cipher := r.StorageValue()
	if bytes.Contains(cipher.Ciphertext, out.StorageValue().Body) || bytes.Contains(cipher.Ciphertext, []byte("private-cookie")) {
		t.Fatal("receipt material is plaintext")
	}
	got, e := s.openReceipt(cipher)
	if e != nil || !bytes.Equal(got.StorageValue().Body, out.StorageValue().Body) {
		t.Fatal("receipt round trip failed")
	}
	cipher.Key = "other"
	if _, e := s.openReceipt(cipher); e != ErrUnavailable {
		t.Fatal("receipt binding not authenticated")
	}
	cipher = r.StorageValue()
	cipher.Ciphertext[len(cipher.Ciphertext)-1] ^= 1
	if _, e := s.openReceipt(cipher); e != ErrUnavailable {
		t.Fatal("receipt tampering accepted")
	}
}
func TestRateMemoryAndConcurrencyAreBounded(t *testing.T) {
	s := testService(t)
	for i := 0; i < 10; i++ {
		if e := s.limit("same"); e != nil {
			t.Fatal(e)
		}
	}
	if e := s.limit("same"); e != ErrRateLimited {
		t.Fatal("per-context limit absent")
	}
	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func(i int) { defer wg.Done(); _ = s.limit(fmt.Sprint(i)) }(i)
	}
	wg.Wait()
	if s.state().global.count != 60 || len(s.state().rates) > 256 {
		t.Fatal("global bound absent")
	}
	s.state().hashing <- struct{}{}
	s.state().hashing <- struct{}{}
	r, e := s.Decode("login", "", "", []byte(`{"schema_version":1,"login_name":"local_user","password":"password"}`))
	if e != nil {
		t.Fatal(e)
	}
	s.state().global = rateEntry{}
	s.state().rates = map[string]rateEntry{}
	if _, e := s.Mutate(context.Background(), BrowserCookie(strings.Repeat("A", 43)), strings.Repeat("A", 43), "a-valid-key-123456", "host", r); e != ErrRateLimited {
		t.Fatal("hash concurrency bound absent")
	}
	<-s.state().hashing
	<-s.state().hashing
}
func TestReadOnlySecretFileBoundary(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "secret")
	if e := os.WriteFile(p, []byte("private"), 0400); e != nil {
		t.Fatal(e)
	}
	if _, e := ReadSecretFile(p, 32); e != nil {
		t.Fatal(e)
	}
	if e := os.Chmod(p, 0600); e != nil {
		t.Fatal(e)
	}
	if _, e := ReadSecretFile(p, 32); e != ErrUnavailable {
		t.Fatal("writable secret accepted")
	}
	_ = os.Chmod(p, 0400)
	link := filepath.Join(dir, "link")
	if e := os.Symlink(p, link); e != nil {
		t.Fatal(e)
	}
	if _, e := ReadSecretFile(link, 32); e != ErrUnavailable {
		t.Fatal("symlink accepted")
	}
	if _, e := ReadSecretFile(p, 3); e != ErrUnavailable {
		t.Fatal("oversized secret accepted")
	}
}
