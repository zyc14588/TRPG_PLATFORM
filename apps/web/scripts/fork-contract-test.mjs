import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

export async function assertCompositeForkContract(root) {
  const campaignOperationsSource = await readFile(
    path.join(root, "dist/src/app/campaign-operations.js"),
    "utf8",
  );
  assert.equal(
    campaignOperationsSource.includes("api.createForkedCampaign(data.parentCampaignId"),
    true,
    "forked Campaign creation must use the composite server command",
  );
  assert.equal(
    campaignOperationsSource.includes("await api.forkCampaign(data.parentCampaignId"),
    false,
    "browser must not create a child and materialize its fork in separate requests",
  );
}
