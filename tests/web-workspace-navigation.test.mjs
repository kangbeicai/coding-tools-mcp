import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const webWorkspacePagePath = new URL(
  "../src/routes/web/workspace/[id]/+page.svelte",
  import.meta.url,
);

test("Web 工作区页面响应动态路由参数并丢弃过期加载", async () => {
  const source = await readFile(webWorkspacePagePath, "utf8");

  assert.doesNotMatch(source, /import \{ onMount \} from "svelte"/);
  assert.match(source, /let loadGeneration = 0/);
  assert.match(source, /async function load\(id = workspaceId\)/);
  assert.match(source, /generation !== loadGeneration \|\| id !== workspaceId/);
  assert.match(source, /\$effect\(\(\) => \{\s*const id = workspaceId;/);
  assert.match(source, /profile = null;\s*void load\(id\);/);
});
