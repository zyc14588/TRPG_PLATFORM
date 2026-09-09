// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

package ipc

import (
	"context"
	"encoding/json"
	"io"
	"os"
	"time"

	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/checkpoint"
	"github.com/zyc14588/TRPG_PLATFORM/internal/luaruntime/profile"
)

// Serve must run in a dedicated process: it clears the environment and lowers
// irreversible process resource limits before constructing the one Session VM.
func Serve(ctx context.Context, input io.Reader, output io.Writer) error {
	os.Clearenv()
	if err := profile.CheckBackend(); err != nil {
		return err
	}
	var engine *profile.Engine
	var limits profile.Limits
	var last uint64
	defer func() {
		if engine != nil {
			engine.Close()
		}
	}()
	for {
		raw, err := ReadFrame(input)
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return ErrProtocol
		}
		var req Request
		if err := checkpoint.StrictDecode(raw, &req, MaxFrameBytes); err != nil {
			return ErrProtocol
		}
		if req.Version != Version || req.ID == 0 || req.ID <= last {
			return ErrProtocol
		}
		last = req.ID
		response := Response{Version: Version, ID: req.ID, Profile: profile.ID, Runtime: profile.RuntimeVersion, PID: os.Getpid(), Result: profile.Result{Audit: profile.Audit{Level: "AUDIT-0", Sequence: req.ID, Kind: req.Operation, Outcome: "PASS"}}}
		var operationErr error
		if req.Operation == "initialize" {
			if engine != nil || req.Config == nil || len(req.Source) > 0 || req.State != nil || req.Saved != nil {
				return ErrProtocol
			}
			limits = req.Config.Limits
			if operationErr = applyProcessLimits(limits); operationErr == nil {
				// Includes library/module setup; a malformed or hostile input cannot
				// keep even an initializing worker alive beyond its wall budget.
				timer := time.AfterFunc(time.Duration(limits.WallMillis)*time.Millisecond, func() { os.Exit(124) })
				engine, operationErr = profile.New(*req.Config)
				timer.Stop()
			}
		} else {
			if engine == nil || req.Config != nil {
				return ErrProtocol
			}
			if operationErr = armCPULimit(limits.CPUSeconds); operationErr != nil {
				return operationErr
			}
			timer := time.AfterFunc(time.Duration(limits.WallMillis)*time.Millisecond, func() { os.Exit(124) })
			switch req.Operation {
			case "execute":
				if req.State != nil || req.Saved != nil {
					return ErrProtocol
				}
				response.Result, operationErr = engine.Execute(ctx, req.Source)
			case "state":
				if req.State == nil || req.Saved == nil || len(req.Source) > 0 {
					return ErrProtocol
				}
				operationErr = engine.SetState(*req.State, *req.Saved)
			case "destroy":
				if len(req.Source) > 0 || req.State != nil || req.Saved != nil {
					return ErrProtocol
				}
				engine.Close()
			default:
				return ErrProtocol
			}
			timer.Stop()
		}
		if operationErr != nil {
			response.Error = profile.Code(operationErr)
			response.Result.Audit.Outcome = response.Error
		}
		response.Result.Audit.Sequence = req.ID
		encoded, err := json.Marshal(response)
		if err != nil {
			return ErrProtocol
		}
		if err := WriteFrame(output, encoded); err != nil {
			return err
		}
		if operationErr != nil && req.Operation == "initialize" {
			return operationErr
		}
		if req.Operation == "destroy" {
			return nil
		}
	}
}
