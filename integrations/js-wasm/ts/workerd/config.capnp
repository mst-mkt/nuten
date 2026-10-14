using Workerd = import "/workerd/workerd.capnp";

const config :Workerd.Config = (
  services = [(name = "main", worker = .worker)],
);

const worker :Workerd.Worker = (
  modules = [
    (name = "worker.js", esModule = embed "worker.js"),
    (name = "workerd.js", esModule = embed "../../dist/workerd.js"),
    (name = "format.js", esModule = embed "../../dist/format.js"),
    (name = "gen/nuten.js", esModule = embed "../../dist/gen/nuten.js"),
    (name = "gen/nuten_bg.wasm", wasm = embed "../../dist/gen/nuten_bg.wasm"),
  ],
  compatibilityDate = "2026-10-01",
);
