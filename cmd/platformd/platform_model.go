// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/credential"
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/model"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// Startup ownership and public request adapters belong to the deployment
// batch. This factory accepts only server-reviewed records and secret-file
// references; it starts no listener and issues no implicit qualification.
func platformModels(ctx context.Context, authority *auth.RoomAuthority, rooms room.Storage, launches launch.Storage, repo *postgres.PlatformAuthRepository, masterKeyFile string, endpoints []model.Endpoint, certificates []model.Certification, defaults map[string]string, caps map[string]model.Limits) (*model.Service, *credential.Vault, error) {
	storage, e := postgres.NewPlatformModelStorage(repo)
	if e != nil {
		return nil, nil, e
	}
	if e = storage.Bootstrap(ctx); e != nil {
		return nil, nil, e
	}
	vault, e := credential.New(masterKeyFile)
	if e != nil {
		return nil, nil, e
	}
	service, e := model.New(model.Options{Authority: authority, Rooms: rooms, Launches: launches, Storage: storage, Vault: vault, Endpoints: endpoints, Certifications: certificates, Defaults: defaults, WorkspaceLimits: caps})
	if e != nil {
		vault.Close()
		return nil, nil, e
	}
	return service, vault, nil
}
