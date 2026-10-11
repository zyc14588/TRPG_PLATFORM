// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/auth"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/httpapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/launch"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/player"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/room"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// B011 owns startup, listeners, secret files and the single server lifetime.
// This private constructor starts none of them. All downstream writers/tasks
// share this exact control and the guarded M2 repository composition.
type platformPlayerOptions struct {
	ctx                         context.Context
	repo                        *postgres.PlatformAuthRepository
	authority                   *auth.RoomAuthority
	roomStorage                 room.Storage
	rooms                       *room.Service
	configurations              []launch.Configuration
	models                      launch.ModelChecker
	policies                    session.PolicyProvider
	descriptions                []player.Description
	schema                      []byte
	origin                      string
	maxSessions, maxConnections int
	shared                      *platformPlayerCoreData
}
type platformPlayerComponents struct {
	data **platformPlayerComponentsData
}
type platformPlayerComponentsData struct {
	storage       *postgres.PlatformPlayerStorage
	launchStorage *postgres.ControlledLaunchStorage
	control       *player.Control
	launches      *launch.Service
	sessions      *session.Service
	players       *player.Service
	handler       http.Handler
}

func (platformPlayerComponents) Format(f fmt.State, _ rune) {
	_, _ = io.WriteString(f, "<private player composition>")
}
func (platformPlayerComponents) MarshalJSON() ([]byte, error) { return nil, auth.ErrDenied }
func (c *platformPlayerComponents) state() *platformPlayerComponentsData {
	if c == nil || c.data == nil {
		return nil
	}
	return *c.data
}
func platformPlayers(o platformPlayerOptions) (*platformPlayerComponents, error) {
	if o.ctx == nil || o.ctx.Err() != nil || o.repo == nil || o.authority == nil || o.roomStorage == nil || o.rooms == nil || o.policies == nil || o.rooms.Authentication() != o.authority.Authentication() {
		return nil, auth.ErrInvalid
	}
	shared := o.shared
	var e error
	if shared == nil {
		shared, e = platformPlayerCore(o.ctx, o.repo)
		if e != nil {
			return nil, e
		}
	}
	if shared.repo != o.repo {
		return nil, auth.ErrInvalid
	}
	storage, guarded, control := shared.storage, shared.guarded, shared.control

	launches, e := launch.New(launch.Options{Context: o.ctx, Authority: o.authority, Rooms: o.roomStorage, Storage: guarded, Configurations: o.configurations, Models: o.models, MaxSessions: o.maxSessions, PlayerControl: control})
	if e != nil {
		return nil, e
	}
	sessionStorage, e := postgres.NewPlatformSessionStorage(o.repo)
	if e != nil {
		_ = launches.Close()
		return nil, e
	}
	sessions, e := session.New(session.Options{Launch: launches, Storage: sessionStorage, Policies: o.policies})
	if e != nil {
		_ = launches.Close()
		return nil, e
	}
	players, e := player.New(player.Options{Context: o.ctx, Authority: o.authority, Launch: launches, Sessions: sessions, Control: control, Descriptions: o.descriptions, Schema: o.schema, MaxConnections: o.maxConnections})
	if e != nil {
		_ = launches.Close()
		return nil, e
	}
	handler, e := httpapi.NewPlayerHandler(players, o.rooms, o.origin)
	if e != nil {
		players.Close()
		_ = launches.Close()
		return nil, e
	}
	d := &platformPlayerComponentsData{storage, guarded, control, launches, sessions, players, handler}
	return &platformPlayerComponents{data: &d}, nil
}
func (c *platformPlayerComponents) close() error {
	if c.state() == nil {
		return nil
	}
	c.state().players.Close()
	return c.state().launches.Close()
}
func platformPlayerTasks(ctx context.Context, repo *postgres.PlatformAuthRepository, c *platformPlayerComponents, policies []task.Policy, decorate func(task.Storage) (task.Storage, error)) (*player.TaskStorage, *session.Continuations, []task.Policy, error) {
	if ctx == nil || ctx.Err() != nil || c.state() == nil {
		return nil, nil, nil, task.ErrInvalid
	}
	base, e := postgres.NewPlatformTaskStorage(postgres.PlatformTaskOptions{Repository: repo, Lease: 15 * time.Second, Lifetime: 10 * time.Minute})
	if e != nil {
		return nil, nil, nil, e
	}
	if e = base.Bootstrap(ctx); e != nil {
		return nil, nil, nil, e
	}
	guarded, e := postgres.NewControlledPlayerTasks(base, c.state().storage)
	if e != nil {
		return nil, nil, nil, e
	}
	var dispatch task.Storage = guarded
	if decorate != nil {
		dispatch, e = decorate(guarded)
		if e != nil {
			return nil, nil, nil, e
		}
	}
	storage, wrapped, e := player.BindTasks(dispatch, c.state().control, policies)
	if e != nil {
		return nil, nil, nil, e
	}
	continuations, e := session.NewContinuations(session.ContinuationOptions{Launch: c.state().launches, Storage: guarded, Policies: policies})
	if e != nil {
		return nil, nil, nil, e
	}
	return storage, continuations, wrapped, nil
}

type platformPlayerCoreData struct {
	storage *postgres.PlatformPlayerStorage
	guarded *postgres.ControlledLaunchStorage
	control *player.Control
	repo    *postgres.PlatformAuthRepository
}

func platformPlayerCore(ctx context.Context, repo *postgres.PlatformAuthRepository) (*platformPlayerCoreData, error) {
	base, e := postgres.NewPlatformLaunchStorage(repo)
	if e != nil {
		return nil, e
	}
	if e = base.Bootstrap(ctx); e != nil {
		return nil, e
	}
	storage, e := postgres.NewPlatformPlayerStorage(repo)
	if e != nil {
		return nil, e
	}
	if e = storage.Bootstrap(ctx); e != nil {
		return nil, e
	}
	guarded, e := postgres.NewControlledLaunchStorage(base, storage)
	if e != nil {
		return nil, e
	}
	control, e := player.NewControl(storage)
	if e != nil {
		return nil, e
	}
	return &platformPlayerCoreData{storage, guarded, control, repo}, nil
}
