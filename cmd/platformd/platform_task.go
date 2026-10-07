// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// One trusted composition connects task dispatch to the existing platformd
// launch coordinator. There is no implicit credential, provider or fake input.
func platformTasks(ctx context.Context, repo *postgres.PlatformAuthRepository, games *launch.Service, policies []task.Policy) (*postgres.PlatformTaskStorage, *session.Continuations, error) {
	storage, e := postgres.NewPlatformTaskStorage(postgres.PlatformTaskOptions{Repository: repo, Lease: 15 * time.Second, Lifetime: 10 * time.Minute})
	if e != nil {
		return nil, nil, e
	}
	if e = storage.Bootstrap(ctx); e != nil {
		return nil, nil, e
	}
	completions, e := session.NewContinuations(session.ContinuationOptions{Launch: games, Storage: storage, Policies: policies})
	if e != nil {
		return nil, nil, e
	}
	return storage, completions, nil
}
