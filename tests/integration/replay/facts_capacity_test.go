//go:build linux && integration

// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package replay_test

import (
	"context"
	"crypto/ed25519"
	"crypto/sha256"
	"errors"
	"fmt"
	"reflect"
	"strings"
	"testing"

	"github.com/zyc14588/TRPG_PLATFORM/internal/eventstore"
	"github.com/zyc14588/TRPG_PLATFORM/internal/hostapi"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/archive"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/extension"
	"github.com/zyc14588/TRPG_PLATFORM/internal/package/install"
	fixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata"
	replayfixture "github.com/zyc14588/TRPG_PLATFORM/internal/session/testdata/replay"
	data "github.com/zyc14588/TRPG_PLATFORM/internal/storage/package"
)

// Only the synthetic operator fixture adds these approved namespaces/schemas
// and named plans before installation. Runtime code receives no new grants.
func factsConfiguration(boundary string) fixtureConfiguration {
	return func(pkg *archive.Package, cfg install.PolicyConfig) (*archive.Package, install.PolicyConfig, map[string]install.Evidence, error) {
		approval := cfg.Artifacts[string(pkg.ArtifactIdentity().Digest())]
		if boundary == "bytes" {
			files := map[string][]byte{}
			for _, entry := range pkg.Entries() {
				files[entry.Path()] = entry.Bytes()
			}
			files["schemas/row.schema.json"] = []byte(`{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"score":{"type":"integer"},"blob":{"type":"string","maxLength":12000}},"required":["score"],"additionalProperties":false}`)
			var err error
			pkg, err = archive.FromFiles(files, pkg.ExactLock(), extension.DefaultSupport)
			if err != nil {
				return nil, cfg, nil, err
			}
			ns := approval.Host.Namespaces[fixture.PackageID+"/docs"]
			ns.Schema.Digest = checkpoint.Hash(files["schemas/row.schema.json"])
			approval.Host.Namespaces[fixture.PackageID+"/docs"] = ns
		}
		approval.Host.Namespaces[fixture.PackageID+"/other"] = approval.Host.Namespaces[fixture.PackageID+"/docs"]
		cfg.Artifacts = map[string]install.Approval{string(pkg.ArtifactIdentity().Digest()): approval}
		if boundary != "quantities" {
			return pkg, cfg, nil, nil
		}
		inputEntry, _ := pkg.Entry("schemas/named_input.schema.json")
		input := install.SchemaReference{PackageID: fixture.PackageID, Path: inputEntry.Path(), Digest: checkpoint.Hash(inputEntry.Bytes()), Seed: checkpoint.Object(map[string]checkpoint.Value{"key": checkpoint.Text("q1"), "delta": checkpoint.Int(1)})}
		for name, plan := range map[string]string{"increase": "quantity-add", "read": "quantity-get"} {
			approval.Host.Named[fixture.PackageID+"/"+name] = install.NamedContract{PackageID: fixture.PackageID, ID: name, Plan: plan, Input: input, Output: approval.Host.Result}
		}
		// These distinct fixture-only attestations exercise actual TrustSigned
		// verification; they are not production keys or a certification service.
		publisherSeed, certificationSeed := sha256.Sum256([]byte("B006 synthetic publisher key")), sha256.Sum256([]byte("B006 synthetic certification key"))
		publisher, certification := ed25519.NewKeyFromSeed(publisherSeed[:]), ed25519.NewKeyFromSeed(certificationSeed[:])
		cfg.Keys = map[string]install.Key{
			"fixture-publisher":     {Public: publisher.Public().(ed25519.PublicKey), Publisher: "example.test", State: "ACTIVE", NotBefore: 1, NotAfter: 4000000000},
			"fixture-certification": {Public: certification.Public().(ed25519.PublicKey), Certification: true, State: "ACTIVE", NotBefore: 1, NotAfter: 4000000000},
		}
		policy, err := install.NewPolicy(cfg)
		if err != nil {
			return nil, cfg, nil, err
		}
		const signedAt = int64(1700000000)
		attest := func(kind, id string, private ed25519.PrivateKey) (*install.Attestation, error) {
			raw, err := policy.SigningBytes(kind, pkg, signedAt)
			if err != nil {
				return nil, err
			}
			return &install.Attestation{KeyID: id, SignedAt: signedAt, Signature: ed25519.Sign(private, raw)}, nil
		}
		p, err := attest("publisher", "fixture-publisher", publisher)
		if err != nil {
			return nil, cfg, nil, err
		}
		c, err := attest("certification", "fixture-certification", certification)
		if err != nil {
			return nil, cfg, nil, err
		}
		evidence := map[string]install.Evidence{string(pkg.ArtifactIdentity().Digest()): {Publisher: p, Certification: c}}
		return pkg, cfg, evidence, nil
	}
}

func factsSource(boundary string) string {
	addition := ""
	switch boundary {
	case "rows":
		addition = `
 if command.type=="replace" then
  host.db.delete("docs","d1");host.db.put("other","overflow",{score=next})
 elseif next==2 then
  for i=1,63 do host.db.put("docs","d"..i,{score=next}) end
 elseif next==3 then
  for i=1,64 do host.db.put("other","o"..i,{score=next}) end
 else host.db.put("other","overflow",{score=next}) end
`
	case "bytes":
		addition = `
 local ns=next==2 and "docs" or "other"
 for i=1,20 do host.db.put(ns,"b"..i,{score=next,blob=string.rep("x",12000)}) end
`
	case "quantities":
		addition = `
 local first,last=1,64
 if next==3 then first,last=65,128 elseif next>3 then first,last=129,129 end
 for i=first,last do assert(host.db.named("increase",{key="q"..i,delta=1})==1) end
`
	}
	source := strings.Replace(replayfixture.FactsSource, " host.db.put(\"docs\",\"one\",{score=next})\n", " host.db.put(\"docs\",\"one\",{score=next})\n"+addition, 1)
	if boundary == "rows" {
		source = strings.Replace(source, `command.type=="fail")`, `command.type=="fail" or command.type=="replace")`, 1)
	}
	if boundary == "quantities" {
		source = strings.Replace(source, `if input.counter>1 then assert(host.db.get("docs","one").score==input.counter) end`, `if input.counter>1 then assert(host.db.get("docs","one").score==input.counter);assert(host.db.named("read",{key="q1",delta=0})==1) end`, 1)
	}
	return source
}

func TestRealCumulativeDataFactsStayRecoverable(t *testing.T) {
	for _, boundary := range []string{"rows", "bytes", "quantities"} {
		t.Run(boundary, func(t *testing.T) {
			e := setupConfigured(t, factsSource(boundary), factsConfiguration(boundary))
			t.Cleanup(func() { e.assertReaped(t) })
			b := e.create(t, "facts-"+boundary)
			r := newRig(t, e, b, e.host, "replace")
			accepted := 2
			if boundary == "bytes" {
				accepted = 1
			}
			for k := 1; k <= accepted; k++ {
				got, err := r.registry.Submit(context.Background(), r.gm, envelope(b, fmt.Sprintf("facts-%d", k), "increment", uint64(k)))
				if err != nil || got.Version != uint64(k+1) {
					t.Fatal("valid prefix failed", err)
				}
			}
			// Reconstruct the last accepted prefix before attacking its next bound.
			prefixSession, prefix, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal("accepted prefix not recoverable", err)
			}
			assertVMFacts(t, prefixSession, int64(accepted+1))
			if err = prefixSession.Close(); err != nil {
				t.Fatal(err)
			}
			if boundary == "rows" && len(prefix.Image.Rows) != 128 {
				t.Fatal("row boundary not reached")
			}
			if boundary == "quantities" && len(prefix.Image.Quantities) != 128 {
				t.Fatal("quantity boundary not reached")
			}
			before := capacityInspection(t, e, b)
			rejected, err := r.registry.Submit(context.Background(), r.gm, envelope(b, "facts-overflow", "increment", uint64(accepted+1)))
			if !errors.Is(err, data.ErrDenied) || !reflect.DeepEqual(rejected, data.Receipt{}) || capacityInspection(t, e, b) != before {
				t.Fatal("overfull facts accepted, wrong boundary, or partial write", err)
			}
			duplicate, err := r.registry.Submit(context.Background(), r.gm, envelope(b, "facts-1", "increment", 1))
			if err != nil || !duplicate.Replayed || duplicate.Version != 2 || capacityInspection(t, e, b) != before {
				t.Fatal("duplicate at capacity", err)
			}
			// Recover a real live VM first, then remove only its derived data.
			// Calling its authorized Host command directly avoids an Actor
			// reactivation repairing the tables before this intended probe.
			live, _, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal(err)
			}
			if err = e.host.DropDerivedData(context.Background(), b); err != nil {
				t.Fatal(err)
			}
			poisoned := capacityInspection(t, e, b)
			if poisoned.ImmutableHash != before.ImmutableHash || poisoned.DerivedHash == before.DerivedHash {
				t.Fatal("missing-data probe did not reach its intended boundary")
			}
			cacheRejected, err := live.Commands.Execute(context.Background(), live.VM.Token(), hostapi.Command{Callback: "command", ID: "facts-overflow-missing-data", Principal: "gm", ExpectedVersion: uint64(accepted + 1), Input: fixture.CommandInput("increment", 1), Time: 1000, Random: []int64{7}, Envelope: &data.EnvelopeMetadata{Seat: "gm", Type: "increment", Correlation: "fixture-case"}})
			if !errors.Is(err, data.ErrDenied) || !reflect.DeepEqual(cacheRejected, data.Receipt{}) || capacityInspection(t, e, b) != poisoned {
				t.Fatal("commit capacity trusted missing mutable data", err)
			}
			if err = live.Close(); err != nil {
				t.Fatal(err)
			}
			fixed, _, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal("immutable prefix did not repair missing data", err)
			}
			assertVMFacts(t, fixed, int64(accepted+1))
			if err = fixed.Close(); err != nil {
				t.Fatal(err)
			}
			if boundary == "rows" {
				// Delete and insert in one command has a bounded net image, even though
				// every namespace and the old whole image were already populated.
				replacement, err := r.registry.Submit(context.Background(), r.gm, envelope(b, "facts-replace", "replace", uint64(accepted+1)))
				if err != nil || replacement.Version != uint64(accepted+2) {
					t.Fatal("atomic replacement was denied", err)
				}
				accepted++
			}
			if err = r.registry.Sleep(context.Background(), r.gm); err != nil {
				t.Fatal(err)
			}
			if err = r.registry.Close(); err != nil {
				t.Fatal(err)
			}
			before = capacityInspection(t, e, b)
			if err = e.host.DropDerived(context.Background(), b); err != nil {
				t.Fatal(err)
			}
			restored, report, err := recoverSession(t, e, b, e.host)
			if err != nil {
				t.Fatal("zero-derived recovery after rejection", err)
			}
			assertVMFacts(t, restored, int64(accepted+1))
			if boundary == "rows" && len(report.Image.Rows) != 128 {
				t.Fatal("replacement changed global bound")
			}
			if boundary == "quantities" && len(report.Image.Quantities) != 128 {
				t.Fatal("lost relational facts")
			}
			if report.Image.Version != uint64(accepted+1) || capacityInspection(t, e, b).ImmutableHash != before.ImmutableHash {
				t.Fatal("recovery changed original history")
			}
			if err = restored.Close(); err != nil {
				t.Fatal(err)
			}
			t.Logf("DATA_CAPACITY_RECOVERED boundary=%s version=%d cursor=%d rows=%d quantities=%d history=%s", boundary, report.Image.Version, report.Image.Cursor, len(report.Image.Rows), len(report.Image.Quantities), eventstore.Digest(report.Image))
		})
	}
}
