// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
package main

import (
	"github.com/zyc14588/TRPG_PLATFORM/internal/ai/gateway"
	"github.com/zyc14588/TRPG_PLATFORM/internal/platform/session"
	"github.com/zyc14588/TRPG_PLATFORM/internal/task"
)

// B011 supplies the approved lifetime, current controller credentials and
// installed-package policies. This composition starts no listener, creates
// no user/worker grant and gives the provider no game repository.
func modelWorker(options task.WorkerOptions, g *gateway.Service, completions *session.Continuations, policies []gateway.PolicyOptions) (*task.Runtime, error) {
	if g == nil || completions == nil || len(policies) < 1 || len(policies) > 128 {
		return nil, task.ErrInvalid
	}
	storage, e := gateway.BindTasks(options.Storage)
	if e != nil {
		return nil, e
	}
	options.Storage = storage
	options.Policies = nil
	for _, p := range policies {
		policy, e := g.Policy(p)
		if e != nil {
			return nil, e
		}
		options.Policies = append(options.Policies, policy)
	}
	options.Post = completions.Post
	return task.NewWorker(options)
}
