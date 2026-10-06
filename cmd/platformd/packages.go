// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package main

import (
	"context"
	"database/sql"

	_ "github.com/jackc/pgx/v5/stdlib"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/object"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// packageServices is an internal composition seam for the next owning batch.
// It adds no public endpoint, room selection, or automatic Session startup.
type packageServices struct {
	Installer    *install.Installer
	Reader       *store.Reader
	HostCommands *postgres.HostRepository
	database     *sql.DB
	objects      *object.Directory
}

func openPackageServices(ctx context.Context, dsn, objectRoot string, options install.Options) (*packageServices, error) {
	db, err := sql.Open("pgx", dsn)
	if err != nil {
		return nil, err
	}
	if err = db.PingContext(ctx); err != nil {
		_ = db.Close()
		return nil, err
	}
	objects, err := object.Open(objectRoot)
	if err != nil {
		_ = db.Close()
		return nil, err
	}
	fail := func(err error) (*packageServices, error) { _ = objects.Close(); _ = db.Close(); return nil, err }
	support := options.Support
	if support.HostAPIMajor == 0 {
		support = extension.DefaultSupport
	}
	repo, err := postgres.New(postgres.Options{DB: db, Objects: objects, Support: support})
	if err != nil {
		return fail(err)
	}
	// Schema/workspace provisioning remains a separate trusted operator action.
	options.Repository = repo
	options.Objects = objects
	options.Support = support
	installer, err := install.New(options)
	if err != nil {
		return fail(err)
	}
	reader, err := store.NewReader(repo, objects, options.Access, support)
	if err != nil {
		return fail(err)
	}
	commands, err := postgres.NewHostRepository(postgres.HostOptions{DB: db})
	if err != nil {
		return fail(err)
	}
	return &packageServices{Installer: installer, Reader: reader, HostCommands: commands, database: db, objects: objects}, nil
}

func (s *packageServices) Close() error {
	objectErr := s.objects.Close()
	dbErr := s.database.Close()
	if objectErr != nil {
		return objectErr
	}
	return dbErr
}
