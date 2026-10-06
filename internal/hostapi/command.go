// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// Package hostapi authorizes data-only callbacks and owns the complete command
// workspace. Only an actual successful database commit publishes its result.
package hostapi

import (
	"context"
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/vm"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/store"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

type Options struct {
	Session         *vm.Session
	Binding         data.Binding
	Packages        map[string]*archive.Package
	Repository      data.Repository
	StateSchema     Schema
	ResultSchema    Schema
	ResultSchemas   map[string]Schema
	Namespaces      map[string]Namespace
	EventSchemas    map[string]Schema
	IntentSchemas   map[string]Schema
	NamedOperations map[string]NamedOperation
	Budget          Budget
	AuditPolicy     AuditPolicy
	Audit           func(data.Audit) error
	Validate        func(context.Context, data.Commit) error
	Fault           func(context.Context, string) error
}
type Service struct {
	options Options
	modules map[string]vm.ModuleIdentity
	root    string
}
type Command struct {
	Callback        string
	ID              string
	Principal       string
	ExpectedVersion uint64
	Input           checkpoint.Value
	Time            int64
	Random          []int64
	Envelope        *data.EnvelopeMetadata
	ToolResults     []checkpoint.Value
}

func New(o Options) (*Service, error) {
	bad := func() (*Service, error) { return nil, profile.Fail(profile.ErrConfiguration) }
	if o.Session == nil || !o.Session.HostEnabled() || o.Repository == nil || o.Audit == nil || o.Validate == nil || o.Budget.Validate() != nil || o.AuditPolicy.validate(time.Now()) != nil {
		return bad()
	}
	if !store.ValidID(o.Binding.Workspace) || !store.ValidID(o.Binding.Session) || o.Binding.Session != o.Session.SessionID() || o.Binding.GraphHash != o.Session.GraphHash() {
		return bad()
	}
	modules := o.Session.ModuleBindings()
	hashes := o.Session.PackageHashes()
	if len(o.Packages) != len(hashes) {
		return bad()
	}
	o.Packages = copyMap(o.Packages)
	for id, h := range hashes {
		p := o.Packages[id]
		if p == nil || string(p.ContentHash()) != h {
			return bad()
		}
	}
	validSchema := func(s Schema) bool {
		h, ok := hashes[s.packageID]
		return ok && h == s.packageHash && checkpoint.IsDigest(s.digest)
	}
	if !validSchema(o.StateSchema) || !validSchema(o.ResultSchema) {
		return bad()
	}
	o.Namespaces = copyMap(o.Namespaces)
	o.EventSchemas = copyMap(o.EventSchemas)
	o.IntentSchemas = copyMap(o.IntentSchemas)
	o.NamedOperations = copyMap(o.NamedOperations)
	o.ResultSchemas = copyMap(o.ResultSchemas)
	for name, schema := range o.ResultSchemas {
		if !profile.IsStandardCallback(name) || !validSchema(schema) {
			return bad()
		}
	}
	for key, n := range o.Namespaces {
		if !validSchema(n.Schema) || key == "" || n.MaxRows < 1 || n.MaxRows > o.Budget.Rows || n.MaxBytes < 1 || n.MaxBytes > o.Budget.DataBytes {
			return bad()
		}
		if !strings.HasPrefix(key, n.Schema.packageID+"/") || !store.ValidID(strings.TrimPrefix(key, n.Schema.packageID+"/")) {
			return bad()
		}
		indices := map[string][]string{}
		for id, path := range n.Indices {
			if !store.ValidID(id) || len(path) > 32 {
				return bad()
			}
			indices[id] = append([]string(nil), path...)
		}
		n.Indices = indices
		o.Namespaces[key] = n
	}
	for _, set := range []map[string]Schema{o.EventSchemas, o.IntentSchemas} {
		for key, s := range set {
			if !validSchema(s) || len(key) <= len(s.packageID)+1 || key[:len(s.packageID)+1] != s.packageID+"/" {
				return bad()
			}
		}
	}
	for key, n := range o.NamedOperations {
		if key != n.PackageID+"/"+n.ID || !store.ValidID(n.ID) || !validSchema(n.Input) || !validSchema(n.Output) || n.Input.packageID != n.PackageID || n.Output.packageID != n.PackageID || (n.Plan != "quantity-get" && n.Plan != "quantity-add") {
			return bad()
		}
	}
	root := ""
	for id, p := range o.Packages {
		lock, err := p.ExactLock().Digest()
		if err == nil && string(lock) == o.Binding.GraphHash {
			root = id
		}
	}
	return &Service{options: o, modules: modules, root: root}, nil
}
func copyMap[T any](m map[string]T) map[string]T {
	r := map[string]T{}
	for k, v := range m {
		r[k] = v
	}
	return r
}
func (s *Service) Execute(ctx context.Context, token vm.Token, c Command) (result data.Receipt, err error) {
	return s.run(ctx, token, c, false)
}

// Read invokes a standard read callback against a locked snapshot, then rolls
// the transaction back. It cannot change the version, receipts or effects.
func (s *Service) Read(ctx context.Context, token vm.Token, c Command) (data.Receipt, error) {
	if c.Callback != "project_view" && c.Callback != "list_legal_actions" && c.Callback != "create_checkpoint" && c.Callback != "restore_checkpoint" && c.Callback != "on_session_restore" && c.Callback != "on_safe_migration_boundary" && c.Callback != "cleanup" {
		return data.Receipt{}, profile.Fail(profile.ErrCapability)
	}
	return s.run(ctx, token, c, true)
}
func (s *Service) run(ctx context.Context, token vm.Token, c Command, readonly bool) (result data.Receipt, err error) {
	o := s.options
	if ctx == nil || !store.ValidID(c.ID) || !store.ValidID(c.Principal) || c.ExpectedVersion == 0 || checkpoint.Validate(c.Input) != nil || len(c.Random) > o.Budget.Callbacks {
		return result, profile.Fail(profile.ErrConfiguration)
	}
	callback := c.Callback
	if callback == "" {
		callback = "command"
	}
	if callback != "command" && (!profile.IsStandardCallback(callback) || callback == "validate_command" || callback == "execute_command") {
		return result, profile.Fail(profile.ErrCapability)
	}
	if c.Envelope != nil && (!store.ValidID(c.Envelope.Seat) || !store.ValidID(c.Envelope.Type) || !store.ValidID(c.Envelope.Correlation)) {
		return result, profile.Fail(profile.ErrConfiguration)
	}
	if len(c.ToolResults) > 32 {
		return result, profile.Fail(profile.ErrBudget)
	}
	for _, v := range c.ToolResults {
		if o.ResultSchema.Validate(v) != nil {
			return result, profile.Fail("SCHEMA_REJECTED")
		}
	}
	inputs := data.Inputs{Callback: callback, Time: c.Time, Random: append([]int64(nil), c.Random...), Command: c.Input, Envelope: c.Envelope, ToolResults: c.ToolResults}
	inputs, e := clone(inputs)
	if e != nil {
		return result, e
	}
	header := data.Header{Binding: o.Binding, Principal: c.Principal, CommandID: c.ID, ExpectedVersion: c.ExpectedVersion, ReadOnly: readonly}
	header.Fingerprint = digest(struct {
		Header data.Header
		Inputs data.Inputs
	}{header, inputs})
	if err = o.Session.AuthorizeToken(token, o.Binding.Session); err != nil {
		return result, err
	}
	ctx, cancel := context.WithTimeout(ctx, time.Duration(o.Session.Limits().WallMillis)*time.Millisecond)
	defer cancel()
	tx, err := o.Repository.Begin(ctx, header)
	if err != nil {
		return result, err
	}
	defer tx.Rollback()
	touched := false
	defer func() {
		if err != nil && touched {
			o.Session.Poison()
			result = data.Receipt{}
		}
	}()
	snapshot := tx.Snapshot()
	if snapshot.Binding != o.Binding {
		return result, profile.Fail(profile.ErrCapability)
	}
	if snapshot.Existing != nil {
		if readonly {
			return result, data.ErrConflict
		}
		if snapshot.Existing.Header != header {
			return result, data.ErrConflict
		}
		original := *snapshot.Existing
		original.Replayed = true
		return original, nil
	}
	if snapshot.Version != c.ExpectedVersion || snapshot.SchemaHash != o.StateSchema.digest || o.StateSchema.Validate(snapshot.State) != nil {
		return result, data.ErrConflict
	}
	state, e := clone(snapshot.State)
	if e != nil {
		return result, e
	}
	w := &workspace{header: header, inputs: inputs, state: state, options: o, modules: s.modules, readonly: readonly, rows: map[string]data.Row{}, quantities: map[string]data.Quantity{}, changed: map[string]data.Row{}, quantityChanges: map[string]data.Quantity{}}
	for _, r := range snapshot.Rows {
		n, ok := o.Namespaces[r.PackageID+"/"+r.Namespace]
		if !ok || r.SchemaHash != n.Schema.digest || n.Schema.Validate(r.Value) != nil {
			return result, profile.Fail("SCHEMA_REJECTED")
		}
		row, e := clone(r)
		if e != nil {
			return result, e
		}
		w.rows[rowKey(r.PackageID, r.Namespace, r.Key)] = row
	}
	for _, q := range snapshot.Quantities {
		if _, ok := o.Packages[q.PackageID]; !ok || q.Table != "quantity" {
			return result, profile.Fail(profile.ErrCapability)
		}
		w.quantities[rowKey(q.PackageID, q.Table, q.Key)] = q
	}
	touched = true
	luaResult, err := o.Session.Invoke(ctx, token, o.Binding.Session, callback, []checkpoint.Value{inputs.Command}, w.call)
	if err != nil {
		return result, err
	}
	if w.failure != nil {
		return result, w.failure
	}
	if len(luaResult.Values) != 1 {
		return result, profile.Fail(profile.ErrValue)
	}
	fault := func(point string) error {
		if o.Fault != nil {
			return o.Fault(ctx, point)
		}
		return ctx.Err()
	}
	if err = fault("after-lua"); err != nil {
		return result, err
	}
	resultSchema := o.ResultSchema
	if schema, ok := o.ResultSchemas[callback]; ok {
		resultSchema = schema
	}
	if o.StateSchema.Validate(w.state) != nil || resultSchema.Validate(luaResult.Values[0]) != nil {
		return result, profile.Fail("SCHEMA_REJECTED")
	}
	if err = fault("after-schema"); err != nil {
		return result, err
	}
	effects := len(w.patches) + len(w.changed) + len(w.quantityChanges) + len(w.tasks) + len(w.continuations) + len(w.outbox)
	if readonly {
		if effects != 0 || len(w.events) != 0 || digest(w.state) != digest(snapshot.State) {
			return result, profile.Fail("READ_EFFECT_DENIED")
		}
		raw, e := nativeJSON(luaResult.Values[0])
		if e != nil || len(raw)+luaResult.OutputBytes+w.outputBytes > o.Budget.OutputBytes {
			return result, profile.Fail(profile.ErrBudget)
		}
		a := data.Audit{Workspace: o.Binding.Workspace, Session: o.Binding.Session, CommandID: c.ID, Level: "AUDIT-0", Operation: callback, Phase: "read", ArgumentsHash: header.Fingerprint, ResultHash: digest(luaResult.Values[0]), Outcome: "READ_VALIDATED", StateVersion: snapshot.Version, CallbackCount: w.callbacks}
		if e := o.Audit(a); e != nil {
			return result, profile.Fail("AUDIT_FAILED")
		}
		return data.Receipt{Header: header, Version: snapshot.Version, Result: luaResult.Values[0], Inputs: inputs}, nil
	}
	if effects > 0 && len(w.events) == 0 {
		return result, profile.Fail("AUTHORITY_EVENT_REQUIRED")
	}
	if inputs.Envelope != nil && len(w.events) == 0 {
		return result, profile.Fail("AUTHORITY_EVENT_REQUIRED")
	}
	// A minimal delivery notification joins every live event-producing command's
	// SQL transaction. It contains no game state or seat secrets.
	if inputs.Envelope != nil && len(w.events) > 0 {
		if s.root == "" || len(w.outbox) >= o.Budget.Outbox {
			return result, profile.Fail(profile.ErrBudget)
		}
		id := "notify-" + strings.TrimPrefix(digest(header), "sha256:")[:32]
		w.outbox = append(w.outbox, data.Intent{ID: id, PackageID: s.root, Kind: "session-notify", Payload: checkpoint.Value{Kind: "table", Table: map[string]checkpoint.Value{"command_id": {Kind: "string", String: c.ID}}}})
	}
	eventID := ""
	if len(w.events) > 0 {
		eventID = w.events[0].ID
	}
	for i := range w.patches {
		w.patches[i].EventID = eventID
	}
	commit := data.Commit{Header: header, State: w.state, SchemaHash: o.StateSchema.digest, Patches: w.patches, Events: w.events, Tasks: w.tasks, Continuations: w.continuations, Outbox: w.outbox, Result: luaResult.Values[0], Inputs: inputs, Audit: w.audits}
	keys := []string{}
	for k := range w.changed {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		commit.Rows = append(commit.Rows, w.changed[k])
	}
	keys = nil
	for k := range w.quantityChanges {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		commit.Quantities = append(commit.Quantities, w.quantityChanges[k])
	}
	proposed, e := clone(commit)
	if e != nil {
		return result, e
	}
	if err = o.Validate(ctx, proposed); err != nil {
		return result, profile.Fail("GO_VALIDATION_FAILED")
	}
	if err = fault("after-go-validation"); err != nil {
		return result, err
	}
	raw, e := nativeJSON(commit.Result)
	outputBytes := len(raw) + luaResult.OutputBytes + w.outputBytes
	if e != nil || outputBytes > o.Budget.OutputBytes {
		return result, profile.Fail(profile.ErrBudget)
	}
	audit := data.Audit{Workspace: o.Binding.Workspace, Session: o.Binding.Session, CommandID: c.ID, Level: "AUDIT-0", Operation: "command-commit", Phase: "commit", ArgumentsHash: header.Fingerprint, ResultHash: digest(commit.Result), Outcome: "VALIDATED_PENDING_COMMIT", StateVersion: c.ExpectedVersion, CallbackCount: w.callbacks, EventID: eventID}
	sealedAudit, _ := clone(audit)
	if err = o.Audit(sealedAudit); err != nil {
		return result, profile.Fail("AUDIT_FAILED")
	}
	audit.Outcome = "COMMITTED"
	commit.Audit = append(commit.Audit, audit)
	if err = fault("before-commit"); err != nil {
		return result, err
	}
	result, err = tx.Commit(ctx, commit)
	if err != nil {
		return data.Receipt{}, err
	}
	// Commit is the visibility boundary. A post-commit VM synchronization problem
	// poisons the cache, while the authoritative committed receipt stays successful.
	if e = o.Session.AcceptCommittedState(result.Version, commit.State); e != nil {
		o.Session.Poison()
	}
	return result, nil
}
func (s *Service) String() string { return fmt.Sprintf("HostService<%s>", s.options.Binding.Session) }

func (s *Service) StateSchemaDigest() string { return s.options.StateSchema.Digest() }
func (s *Service) CallbackSchemaDigest(name string) string {
	if schema, ok := s.options.ResultSchemas[name]; ok {
		return schema.Digest()
	}
	return s.options.ResultSchema.Digest()
}
