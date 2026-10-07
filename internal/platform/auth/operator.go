// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package auth

import (
	"context"
	"io"
	"os"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/core"
)

// ReadSecretFile rejects symlinks, writable/world-readable files, replacements
// during open, and oversized input. Errors never include a path or OS cause.
func ReadSecretFile(path string, maximum int64) (Secret[[]byte], error) {
	if maximum <= 0 || maximum > 16384 {
		return Secret[[]byte]{}, ErrInvalid
	}
	before, e := os.Lstat(path)
	if e != nil || !before.Mode().IsRegular() || before.Mode().Perm()&0227 != 0 {
		return Secret[[]byte]{}, ErrUnavailable
	}
	f, e := os.Open(path)
	if e != nil {
		return Secret[[]byte]{}, ErrUnavailable
	}
	defer f.Close()
	after, e := f.Stat()
	if e != nil || !os.SameFile(before, after) || !after.Mode().IsRegular() || after.Mode().Perm()&0227 != 0 {
		return Secret[[]byte]{}, ErrUnavailable
	}
	data, e := io.ReadAll(io.LimitReader(f, maximum+1))
	if e != nil || int64(len(data)) > maximum {
		clear(data)
		return Secret[[]byte]{}, ErrUnavailable
	}
	return protect(data), nil
}

// BootstrapFromFiles never enables open registration. A seed creates only the
// first account, under the same storage lock as every authentication mutation.
// Re-reading a consumed grant cannot reactivate it.
func (s *Service) BootstrapFromFiles(ctx context.Context, seedFile, grantsFile string) error {
	var seed map[string]any
	var grants []any
	if seedFile != "" {
		secret, e := ReadSecretFile(seedFile, 16384)
		if e != nil {
			return e
		}
		raw := secret.StorageValue()
		defer clear(raw)
		v, e := strictJSON(raw)
		if e != nil {
			return e
		}
		var ok bool
		seed, ok = v.(map[string]any)
		if !ok || len(seed) != 3 {
			return ErrInvalid
		}
		for _, key := range []string{"login_name", "password", "display_name"} {
			if _, ok := seed[key].(string); !ok {
				return ErrInvalid
			}
		}
		fields := map[string]any{"schema_version": 1, "mode": "new_account", "login_name": seed["login_name"], "password": seed["password"], "display_name": seed["display_name"]}
		if e := s.state().schemas["GuestClaimRequest"].Validate(fields); e != nil {
			return ErrInvalid
		}
		seed = fields
	}
	if grantsFile != "" {
		secret, e := ReadSecretFile(grantsFile, 16384)
		if e != nil {
			return e
		}
		raw := secret.StorageValue()
		defer clear(raw)
		v, e := strictJSON(raw)
		if e != nil {
			return e
		}
		var ok bool
		grants, ok = v.([]any)
		if !ok || len(grants) > 128 {
			return ErrInvalid
		}
	}
	if seed == nil && grants == nil {
		return nil
	}
	d := s.state()
	if d == nil {
		return ErrUnavailable
	}
	select {
	case d.hashing <- struct{}{}:
		defer func() { <-d.hashing }()
	default:
		return ErrRateLimited
	}
	return s.transact(ctx, func(tx Transaction) error {
		if seed != nil {
			count, e := tx.AccountCount(ctx)
			if e != nil {
				return SafeError(e)
			}
			if count == 0 {
				if _, e := createAccount(ctx, tx, RequestData{Fields: seed}); e != nil {
					return e
				}
			}
		}
		now, e := tx.Core().Now(ctx)
		if e != nil {
			return SafeError(e)
		}
		for _, item := range grants {
			m, ok := item.(map[string]any)
			if !ok || len(m) != 2 {
				return ErrInvalid
			}
			raw, ok := m["token"].(string)
			if !ok || !tokenID.MatchString(raw) {
				return ErrInvalid
			}
			expires, ok := m["expires_at"].(string)
			if !ok {
				return ErrInvalid
			}
			expiry, e := time.Parse(time.RFC3339Nano, expires)
			if e != nil || !expiry.After(now) {
				return ErrInvalid
			}
			if e := tx.PutGrant(ctx, tokenHash(raw), expiry.UTC()); e != nil {
				return SafeError(e)
			}
		}
		return nil
	})
}

// Revoke is a trusted security/operator seam, never an HTTP identity assertion.
func (s *Service) Revoke(ctx context.Context, cookie BrowserCredential) error {
	return s.transact(ctx, func(tx Transaction) error {
		v, e := s.findSession(ctx, tx, cookie)
		if e != nil {
			return e
		}
		v.Revoked = true
		return tx.PutSession(ctx, StoredSession(v))
	})
}

var _ core.Repository = singleTransaction{}
