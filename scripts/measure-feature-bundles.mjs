import { fileURLToPath } from "node:url";
import { checkBundleSize } from "./check-bundle-size.mjs";

export function measureFeatureBundles() {
  return checkBundleSize({ quiet: true }).featureGraphs.map((graph) => ({
    feature: graph.feature,
    entryName: graph.entryName,
    bytes: graph.bytes,
    gzipBytes: graph.gzipBytes,
    maximumBytes: graph.maximumBytes,
    maximumGzipBytes: graph.maximumGzipBytes,
    entries: graph.entries,
  }));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  console.log(JSON.stringify({
    schemaVersion: 1,
    features: measureFeatureBundles(),
  }, null, 2));
}
