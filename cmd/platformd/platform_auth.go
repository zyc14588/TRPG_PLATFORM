// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"net/http"
	"os"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// platformAuthentication is the server composition seam consumed by the later
// platform startup gate. Constructing it does not start a process or listener.
// No untrusted forwarded-header or arbitrary bearer identity is accepted.
func platformAuthentication(ctx context.Context, origin, dsnFile, cookieKeyFile, replayKeyFile, schemaFile, seedFile, grantsFile string) (http.Handler, func() error, error) {
	dsn, e := auth.ReadSecretFile(dsnFile, 16384)
	if e != nil {
		return nil, nil, e
	}
	defer clear(dsn.StorageValue())
	cookie, e := auth.ReadSecretFile(cookieKeyFile, 32)
	if e != nil {
		return nil, nil, e
	}
	defer clear(cookie.StorageValue())
	replay, e := auth.ReadSecretFile(replayKeyFile, 32)
	if e != nil {
		return nil, nil, e
	}
	defer clear(replay.StorageValue())
	schema, e := os.ReadFile(schemaFile)
	if e != nil || len(schema) > 65536 {
		return nil, nil, auth.ErrUnavailable
	}
	r, e := postgres.OpenPlatformAuthRepository(ctx, string(dsn.StorageValue()), nil)
	if e != nil {
		return nil, nil, e
	}
	closeOnFailure := func(e error) (http.Handler, func() error, error) { _ = r.Close(); return nil, nil, auth.SafeError(e) }
	if e := r.Bootstrap(ctx); e != nil {
		return closeOnFailure(e)
	}
	s, e := auth.NewService(r, cookie.StorageValue(), replay.StorageValue(), schema, nil)
	if e != nil {
		return closeOnFailure(e)
	}
	if e := s.BootstrapFromFiles(ctx, seedFile, grantsFile); e != nil {
		return closeOnFailure(e)
	}
	h, e := httpapi.NewHandler(s, origin)
	if e != nil {
		return closeOnFailure(e)
	}
	return h, r.Close, nil
}
