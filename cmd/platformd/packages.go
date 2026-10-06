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
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
	"github.com/zyc14588/TRPG_PLATFORM/internal/storage/postgres"
)

// packageServices is an internal composition seam for the next owning batch.
// It adds no public endpoint, room selection, or automatic Session startup.
type packageServices struct {
	Installer    *install.Installer
	Reader       *store.Reader
	HostCommands *postgres.HostRepository
	Sessions     *install.SessionFactory
	installation *postgres.Repository
	database     *sql.DB
	objects      *object.Directory
}

type sessionValidation struct {
	Audit    func(data.Audit) error
	Validate func(context.Context, data.Commit) error
	Fault    func(context.Context, string) error // explicit internal operator fixture only
}

func openPackageServices(ctx context.Context, dsn, objectRoot string, options install.Options, validation ...sessionValidation) (*packageServices, error) {
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
	if len(validation) > 1 {
		return fail(install.ErrPolicy)
	}
	var fault func(context.Context, string) error
	if len(validation) == 1 {
		fault = validation[0].Fault
	}
	commands, err := postgres.NewHostRepository(postgres.HostOptions{DB: db, Fault: fault})
	if err != nil {
		return fail(err)
	}
	var sessions *install.SessionFactory
	if len(validation) > 1 {
		return fail(install.ErrPolicy)
	}
	if len(validation) == 1 {
		sessions, err = install.NewSessionFactory(install.SessionOptions{Reader: reader, Policy: options.Policy, Repository: commands, Runtime: options.Runtime, Execution: options.Execution, Audit: validation[0].Audit, Validate: validation[0].Validate})
		if err != nil {
			return fail(err)
		}
	}
	return &packageServices{Installer: installer, Reader: reader, HostCommands: commands, Sessions: sessions, installation: repo, database: db, objects: objects}, nil
}

func (s *packageServices) Close() error {
	objectErr := s.objects.Close()
	dbErr := s.database.Close()
	if objectErr != nil {
		return objectErr
	}
	return dbErr
}
