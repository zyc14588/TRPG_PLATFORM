/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

import { useState } from 'react'

import {
  creatorBridge,
  type CreatorBridge,
  type ExtensionInspection,
  type Inspection,
} from './bridge'
import {
  applyEditResult,
  applyExportResult,
  exportCreatorArchive,
  importCreatorArchive,
  inspectCreatorSession,
  reimportCreatorExport,
  validateAndApplyCreatorEdit,
} from './workflow'

export interface Diagnostic {
  kind: 'error' | 'success'
  code?: string
  detail: string
}

interface AppProps {
  bridge?: CreatorBridge
  initialInspection?: Inspection | null
  initialDiagnostic?: Diagnostic | null
}

interface DescriptorPickerProps {
  extensions: ExtensionInspection[]
  selectedNamespace: string
  busy: boolean
  onSelect(namespace: string): void
}

interface ExtensionDetailsProps {
  extension: ExtensionInspection
}

interface EditableDocumentProps {
  value: string
  busy: boolean
  onChange(value: string): void
  onValidate(): void
}

interface ReadOnlyDocumentProps {
  extension: ExtensionInspection
}

const emptyExtensions: ExtensionInspection[] = []
const diagnosticLimit = 320
const contractCodePattern = /^(ERR_[A-Z0-9_]{1,96})(?::|\s)/

function boundedText(value: string): string {
  return value.length <= diagnosticLimit ? value : `${value.slice(0, diagnosticLimit)}…`
}

export function diagnosticFromError(value: unknown): Diagnostic {
  if (typeof value === 'object' && value !== null) {
    const candidate = value as { code?: unknown; detail?: unknown; message?: unknown }
    const code = typeof candidate.code === 'string' ? boundedText(candidate.code) : undefined
    const detail =
      typeof candidate.detail === 'string'
        ? boundedText(candidate.detail)
        : typeof candidate.message === 'string'
          ? diagnosticFromMessage(candidate.message).detail
          : 'Creator operation failed'
    if (code !== undefined) {
      return { kind: 'error', code, detail }
    }
    if (typeof candidate.message === 'string') {
      return diagnosticFromMessage(candidate.message)
    }
    return { kind: 'error', detail }
  }
  if (typeof value === 'string') {
    return diagnosticFromMessage(value)
  }
  return { kind: 'error', detail: 'Creator operation failed' }
}

function diagnosticFromMessage(value: string): Diagnostic {
  const bounded = boundedText(value)
  const match = contractCodePattern.exec(bounded)
  if (match === null) {
    return { kind: 'error', detail: bounded }
  }
  const detail = bounded.slice(match[0].length).replace(/^\s+/, '') || 'Creator operation failed'
  return { kind: 'error', code: match[1], detail }
}

export function DiagnosticBanner({ diagnostic }: { diagnostic: Diagnostic | null }) {
  if (diagnostic === null) {
    return <div className="diagnostic diagnostic-empty" aria-live="polite" />
  }
  const content = diagnostic.code === undefined ? diagnostic.detail : `${diagnostic.code}: ${diagnostic.detail}`
  return (
    <div
      className={`diagnostic diagnostic-${diagnostic.kind}`}
      role={diagnostic.kind === 'error' ? 'alert' : 'status'}
      aria-live="polite"
    >
      {content}
    </div>
  )
}

function DescriptorPicker({ extensions, selectedNamespace, busy, onSelect }: DescriptorPickerProps) {
  return (
    <div className="field-group">
      <label htmlFor="extension-picker">Declared extension</label>
      <select
        id="extension-picker"
        data-control="extension-picker"
        value={selectedNamespace}
        disabled={busy || extensions.length === 0}
        onChange={(event) => onSelect(event.currentTarget.value)}
      >
        {extensions.length === 0 ? <option value="">Import an archive to inspect descriptors</option> : null}
        {extensions.map((extension) => (
          <option key={extension.descriptor.namespace} value={extension.descriptor.namespace}>
            {extension.descriptor.namespace} · {extension.status}
          </option>
        ))}
      </select>
    </div>
  )
}

function ExtensionDetails({ extension }: ExtensionDetailsProps) {
  const descriptor = extension.descriptor
  return (
    <dl className="descriptor-grid" aria-label="Selected extension contract">
      <div>
        <dt>Namespace</dt>
        <dd>{descriptor.namespace}</dd>
      </div>
      <div>
        <dt>Status</dt>
        <dd>{extension.status}</dd>
      </div>
      <div>
        <dt>Schema digest</dt>
        <dd>{descriptor.schema_sha256}</dd>
      </div>
      <div>
        <dt>Schema path</dt>
        <dd>{descriptor.schema_path}</dd>
      </div>
      <div>
        <dt>Payload path</dt>
        <dd>{descriptor.payload_path}</dd>
      </div>
      <div>
        <dt>Contract</dt>
        <dd>
          v{descriptor.contract_version} · {descriptor.required ? 'required' : 'optional'}
        </dd>
      </div>
      <div>
        <dt>Host compatibility</dt>
        <dd>
          {descriptor.host_api_major}.{descriptor.host_api_min_minor}–{descriptor.host_api_max_minor}
        </dd>
      </div>
    </dl>
  )
}

function EditableDocument({ value, busy, onChange, onValidate }: EditableDocumentProps) {
  return (
    <section className="editor-panel" aria-labelledby="editor-title">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Supported contract</p>
          <h2 id="editor-title">Canonical JSON value</h2>
        </div>
        <span className="status-chip status-editable">Editable</span>
      </div>
      <label className="sr-only" htmlFor="json-editor">
        Extension JSON
      </label>
      <textarea
        id="json-editor"
        data-control="json-editor"
        spellCheck={false}
        value={value}
        disabled={busy}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
      <div className="button-row">
        <button
          type="button"
          className="button button-primary"
          data-control="validate-edit"
          disabled={busy}
          onClick={onValidate}
        >
          Validate &amp; apply edit
        </button>
      </div>
    </section>
  )
}

function ReadOnlyDocument({ extension }: ReadOnlyDocumentProps) {
  return (
    <section className="readonly-panel" aria-labelledby="readonly-title">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Optional unsupported contract</p>
          <h2 id="readonly-title">Preserved read-only</h2>
        </div>
        <span className="status-chip status-readonly">Read-only</span>
      </div>
      <p>{extension.read_only_reason ?? 'This contract is unavailable to the current Host.'}</p>
      <dl className="readonly-metadata">
        <div>
          <dt>Raw payload digest</dt>
          <dd>{extension.raw_payload_sha256 ?? 'Unavailable'}</dd>
        </div>
        <div>
          <dt>Raw payload size</dt>
          <dd>{extension.raw_payload_bytes ?? 0} bytes</dd>
        </div>
      </dl>
      <p className="boundary-note">Opaque bytes remain preserved; this editor does not interpret or modify them.</p>
    </section>
  )
}

function initialNamespace(inspection: Inspection | null | undefined): string {
  return inspection?.extensions[0]?.descriptor.namespace ?? ''
}

function initialJSON(inspection: Inspection | null | undefined): string {
  return inspection?.extensions[0]?.canonical_json ?? ''
}

export function App({
  bridge = creatorBridge,
  initialInspection = null,
  initialDiagnostic = null,
}: AppProps) {
  const [archivePath, setArchivePath] = useState(initialInspection?.source_path ?? '')
  const [outputPath, setOutputPath] = useState('')
  const [inspection, setInspection] = useState<Inspection | null>(initialInspection)
  const [selectedNamespace, setSelectedNamespace] = useState(() => initialNamespace(initialInspection))
  const [editorJSON, setEditorJSON] = useState(() => initialJSON(initialInspection))
  const [lastOutputPath, setLastOutputPath] = useState('')
  const [busyAction, setBusyAction] = useState<string | null>(null)
  const [diagnostic, setDiagnostic] = useState<Diagnostic | null>(initialDiagnostic)

  const extensions = inspection?.extensions ?? emptyExtensions
  const selectedExtension =
    extensions.find((extension) => extension.descriptor.namespace === selectedNamespace) ??
    extensions[0] ??
    null
  const isBusy = busyAction !== null

  function acceptInspection(next: Inspection, message: string) {
    const namespace = initialNamespace(next)
    setInspection(next)
    setArchivePath(next.source_path)
    setSelectedNamespace(namespace)
    setEditorJSON(initialJSON(next))
    setDiagnostic({ kind: 'success', detail: message })
  }

  function handleSelect(namespace: string) {
    const next = extensions.find((extension) => extension.descriptor.namespace === namespace)
    setSelectedNamespace(namespace)
    setEditorJSON(next?.canonical_json ?? '')
    setDiagnostic(null)
  }

  async function handleImport() {
    if (isBusy) return
    if (archivePath.trim() === '') {
      setDiagnostic({ kind: 'error', code: 'ERR_CREATOR_INPUT', detail: 'Enter an archive path.' })
      return
    }
    setBusyAction('import')
    setDiagnostic(null)
    try {
      const next = await importCreatorArchive(bridge, archivePath)
      setLastOutputPath('')
      acceptInspection(next, 'Archive imported and descriptors inspected.')
    } catch (error) {
      setDiagnostic(diagnosticFromError(error))
    } finally {
      setBusyAction(null)
    }
  }

  async function handleInspect() {
    if (isBusy) return
    setBusyAction('inspect')
    setDiagnostic(null)
    try {
      const next = await inspectCreatorSession(bridge)
      acceptInspection(next, 'Descriptor inspection refreshed.')
    } catch (error) {
      setDiagnostic(diagnosticFromError(error))
    } finally {
      setBusyAction(null)
    }
  }

  async function handleValidateEdit() {
    if (isBusy || inspection === null || selectedExtension === null || !selectedExtension.editable) return
    setBusyAction('edit')
    setDiagnostic(null)
    try {
      const result = await validateAndApplyCreatorEdit(bridge, inspection, selectedExtension, editorJSON)
      setEditorJSON(result.canonical_json)
      setInspection((current) =>
        current === null ? current : applyEditResult(current, result),
      )
      setDiagnostic({ kind: 'success', detail: 'JSON is valid and the edit is applied to this session.' })
    } catch (error) {
      setDiagnostic(diagnosticFromError(error))
    } finally {
      setBusyAction(null)
    }
  }

  async function handleExport() {
    if (isBusy || inspection === null) return
    if (outputPath.trim() === '') {
      setDiagnostic({ kind: 'error', code: 'ERR_CREATOR_INPUT', detail: 'Enter an export path.' })
      return
    }
    setBusyAction('export')
    setDiagnostic(null)
    try {
      const result = await exportCreatorArchive(bridge, inspection, outputPath)
      setLastOutputPath(result.path)
      setArchivePath(result.path)
      setInspection((current) =>
        current === null ? current : applyExportResult(current, result),
      )
      setDiagnostic({ kind: 'success', detail: 'Archive exported atomically. Re-import it to confirm the saved model.' })
    } catch (error) {
      setDiagnostic(diagnosticFromError(error))
    } finally {
      setBusyAction(null)
    }
  }

  async function handleReimport() {
    if (isBusy || lastOutputPath === '') return
    setBusyAction('reimport')
    setDiagnostic(null)
    try {
      const next = await reimportCreatorExport(bridge, lastOutputPath)
      acceptInspection(next, 'Exported archive re-imported and inspected.')
    } catch (error) {
      setDiagnostic(diagnosticFromError(error))
    } finally {
      setBusyAction(null)
    }
  }

  return (
    <main className="studio-shell" aria-labelledby="studio-title" aria-busy={isBusy}>
      <header className="studio-header">
        <div>
          <p className="eyebrow">Package extensions</p>
          <h1 id="studio-title">Schema-validated generic JSON</h1>
          <p className="studio-intro">
            Inspect declared contracts, apply validated JSON edits, and re-import deterministic exports.
          </p>
        </div>
        <div className="header-boundary" aria-label="Creator boundary">
          <span>Package-generic JSON by design</span>
          <span>No game-specific forms or templates</span>
          <span>No playable preview or runtime execution</span>
        </div>
      </header>

      <DiagnosticBanner diagnostic={diagnostic} />

      <section className="workspace-card" aria-labelledby="import-title">
        <div className="section-heading">
          <div>
            <p className="step-number">01</p>
            <h2 id="import-title">Import and inspect</h2>
          </div>
          {inspection === null ? null : <span className="content-hash">{inspection.content_hash}</span>}
        </div>
        <div className="path-actions">
          <div className="field-group field-grow">
            <label htmlFor="archive-path">Package archive path</label>
            <input
              id="archive-path"
              data-control="archive-path"
              value={archivePath}
              disabled={isBusy}
              onChange={(event) => setArchivePath(event.currentTarget.value)}
            />
          </div>
          <button
            type="button"
            className="button button-primary"
            data-control="import-archive"
            disabled={isBusy}
            onClick={handleImport}
          >
            {busyAction === 'import' ? 'Importing…' : 'Import archive'}
          </button>
          <button
            type="button"
            className="button button-secondary"
            data-control="refresh-inspection"
            disabled={isBusy || inspection === null}
            onClick={handleInspect}
          >
            Refresh inspection
          </button>
        </div>
        <DescriptorPicker
          extensions={extensions}
          selectedNamespace={selectedExtension?.descriptor.namespace ?? ''}
          busy={isBusy}
          onSelect={handleSelect}
        />
        {selectedExtension === null ? (
          <p className="empty-state">Import an archive to list its declared extension descriptors.</p>
        ) : (
          <ExtensionDetails extension={selectedExtension} />
        )}
      </section>

      {selectedExtension === null ? null : selectedExtension.editable ? (
        <EditableDocument
          value={editorJSON}
          busy={isBusy}
          onChange={setEditorJSON}
          onValidate={handleValidateEdit}
        />
      ) : (
        <ReadOnlyDocument extension={selectedExtension} />
      )}

      <section className="workspace-card" aria-labelledby="export-title">
        <div className="section-heading">
          <div>
            <p className="step-number">03</p>
            <h2 id="export-title">Export and re-import</h2>
          </div>
        </div>
        <div className="path-actions">
          <div className="field-group field-grow">
            <label htmlFor="output-path">Output archive path</label>
            <input
              id="output-path"
              data-control="output-path"
              value={outputPath}
              disabled={isBusy}
              onChange={(event) => setOutputPath(event.currentTarget.value)}
            />
          </div>
          <button
            type="button"
            className="button button-primary"
            data-control="export-archive"
            disabled={isBusy || inspection === null}
            onClick={handleExport}
          >
            {busyAction === 'export' ? 'Exporting…' : 'Export archive'}
          </button>
          <button
            type="button"
            className="button button-secondary"
            data-control="reimport-export"
            disabled={isBusy || lastOutputPath === ''}
            onClick={handleReimport}
          >
            {busyAction === 'reimport' ? 'Re-importing…' : 'Re-import exported archive'}
          </button>
        </div>
        <p className="boundary-note">
          Export writes through the shared service. Re-import is an explicit verification step.
        </p>
      </section>
    </main>
  )
}
