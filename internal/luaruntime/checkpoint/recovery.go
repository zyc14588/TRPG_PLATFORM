// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package checkpoint

// RecoveryBinding adds the approved schema/artifact and actual runner identity
// to the existing checkpoint binding. It is internal derived-cache metadata.
type RecoveryBinding struct {
	Session          Binding `json:"session"`
	Workspace        string  `json:"workspace"`
	ArtifactsHash    string  `json:"artifacts_hash"`
	StateSchema      string  `json:"state_schema"`
	EventSchemas     string  `json:"event_schemas"`
	CheckpointSchema string  `json:"checkpoint_schema"`
	RunnerHash       string  `json:"runner_hash"`
}

func (b RecoveryBinding) Validate() error {
	if b.Session.Validate() != nil || b.Session.StateVersion == 0 || b.Workspace == "" || len(b.Workspace) > 128 || !IsDigest(b.ArtifactsHash) || !IsDigest(b.StateSchema) || !IsDigest(b.EventSchemas) || !IsDigest(b.CheckpointSchema) || !IsDigest(b.RunnerHash) {
		return ErrRejected
	}
	return nil
}
