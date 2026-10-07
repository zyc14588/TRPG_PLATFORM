// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// The production startup batch owns one invocation and the server lifetime.
// Configurations are verified server records; browser and Lua inputs cannot
// supply an installation credential, factory, model verifier or Actor registry.
// This seam adds no listener, public endpoint or automatically started game.
func platformLaunches(ctx context.Context, authority *auth.RoomAuthority, rooms room.Storage, repo *postgres.PlatformAuthRepository, configurations []launch.Configuration, models launch.ModelChecker, maxSessions int) (*launch.Service, error) {
	storage, e := postgres.NewPlatformLaunchStorage(repo)
	if e != nil {
		return nil, e
	}
	if e = storage.Bootstrap(ctx); e != nil {
		return nil, e
	}
	return launch.New(launch.Options{Context: ctx, Authority: authority, Rooms: rooms, Storage: storage, Configurations: configurations, Models: models, MaxSessions: maxSessions})
}
