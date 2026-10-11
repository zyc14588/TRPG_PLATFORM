// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"net/http"
	"os"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// platformRooms composes the approved local authentication and room services.
// Actual startup, Session launch and Compose are provided by later batches.
type platformRoomComponents struct {
	repo      *postgres.PlatformAuthRepository
	storage   *postgres.PlatformRoomStorage
	authority *auth.RoomAuthority
	rooms     *room.Service
	handler   http.Handler
}

func platformRooms(ctx context.Context, origin, dsnFile, cookieKeyFile, replayKeyFile, invitationKeyFile, authSchemaFile, roomSchemaFile, seedFile, grantsFile string) (http.Handler, func() error, error) {
	c, e := platformRoomCore(ctx, origin, dsnFile, cookieKeyFile, replayKeyFile, invitationKeyFile, authSchemaFile, roomSchemaFile, seedFile, grantsFile)
	if e != nil {
		return nil, nil, e
	}
	return c.handler, c.repo.Close, nil
}
func platformRoomCore(ctx context.Context, origin, dsnFile, cookieKeyFile, replayKeyFile, invitationKeyFile, authSchemaFile, roomSchemaFile, seedFile, grantsFile string) (*platformRoomComponents, error) {
	dsn, e := auth.ReadSecretFile(dsnFile, 16384)
	if e != nil {
		return nil, e
	}
	defer clear(dsn.StorageValue())
	cookie, e := auth.ReadSecretFile(cookieKeyFile, 32)
	if e != nil {
		return nil, e
	}
	defer clear(cookie.StorageValue())
	replay, e := auth.ReadSecretFile(replayKeyFile, 32)
	if e != nil {
		return nil, e
	}
	defer clear(replay.StorageValue())
	invite, e := auth.ReadSecretFile(invitationKeyFile, 32)
	if e != nil {
		return nil, e
	}
	defer clear(invite.StorageValue())
	as, e := os.ReadFile(authSchemaFile)
	if e != nil || len(as) > 65536 {
		return nil, auth.ErrUnavailable
	}
	rs, e := os.ReadFile(roomSchemaFile)
	if e != nil || len(rs) > 65536 {
		return nil, auth.ErrUnavailable
	}
	repo, e := postgres.OpenPlatformAuthRepository(ctx, string(dsn.StorageValue()), nil)
	if e != nil {
		return nil, e
	}
	fail := func(e error) (*platformRoomComponents, error) {
		_ = repo.Close()
		return nil, auth.SafeError(e)
	}
	if e = repo.Bootstrap(ctx); e != nil {
		return fail(e)
	}
	store, e := postgres.NewPlatformRoomStorage(repo)
	if e != nil {
		return fail(e)
	}
	if e = store.Bootstrap(ctx); e != nil {
		return fail(e)
	}
	verifier, e := room.NewAdmissionVerifier(store, invite.StorageValue())
	if e != nil {
		return fail(e)
	}
	authority, e := auth.NewRoomAuthority(repo, cookie.StorageValue(), replay.StorageValue(), invite.StorageValue(), as, verifier, verifier)
	if e != nil {
		return fail(e)
	}
	if e = authority.Authentication().BootstrapFromFiles(ctx, seedFile, grantsFile); e != nil {
		return fail(e)
	}
	rooms, e := room.NewService(authority, store, invite.StorageValue(), rs)
	if e != nil {
		return fail(e)
	}
	h, e := httpapi.NewRoomHandler(rooms, origin)
	if e != nil {
		return fail(e)
	}
	return &platformRoomComponents{repo, store, authority, rooms, h}, nil
}
