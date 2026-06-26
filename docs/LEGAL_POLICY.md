# Legal Policy — Rules, RAG, and Creator Content

## Default Rule

The platform must not rely on informal fair-use assumptions for automated ingestion, bundling, or long text output. Legal status must be represented in code and metadata.

## Source Classes

- `official_srd`: official SRD/open rules release; allowed when license metadata is recorded.
- `open_license`: CC0, CC-BY, ORC, OGL, or equivalent recognized open permission; allowed when attribution metadata is recorded.
- `user_upload`: allowed only for the user's room when the user declares they have rights.
- `commercial_adapter`: code/schema/mechanics adapter only; must not contain commercial rule text.
- `unknown`: default to `pending_review`.

## License Status

- `allowed`: eligible for chunking, embedding, indexing, and evidence retrieval.
- `pending_review`: stored only in a review queue or metadata record; not indexed.
- `denied`: not indexed and not redistributed.

## Commercial Rules

Commercial rule support means adapters, schemas, dice policies, and installation hooks. It does not mean bundling, scraping, or redistributing protected rulebook text. Tests for commercial adapters must use invented fixtures or explicitly licensed text.

## Output Limits

RAG evidence should be short previews with citation metadata. Agent outputs must not reproduce large blocks of copyrighted rule text. When evidence is insufficient, Agents should report uncertainty and ask for authorized material or KP confirmation.

## Creator Images

Creator image generation is draft-first, off by default, manually approved, and audited for cost and license. Raw prompts and generated assets must respect room visibility and may not be shown to PL clients when they contain KP-only material.
