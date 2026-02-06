const http = require("http");
const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

const ROOT = path.resolve(__dirname, "..", "www");
const OUT_DIR = path.resolve(__dirname, "..", "output", "web-game-http");
const CLIENT = "C:/Users/Martin/.codex/skills/develop-web-game/scripts/web_game_playwright_client.js";
const ACTIONS = "C:/Users/Martin/.codex/skills/develop-web-game/references/action_payloads.json";
const URL = "http://127.0.0.1:8080/?headless_sim=1";

const MIME = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".css": "text/css",
  ".json": "application/json",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".svg": "image/svg+xml",
};

function serveFile(reqPath, res) {
  const rel = reqPath === "/" ? "/index.html" : reqPath;
  const full = path.normalize(path.join(ROOT, rel));
  if (!full.startsWith(ROOT)) {
    res.statusCode = 403;
    res.end("forbidden");
    return;
  }
  fs.readFile(full, (err, data) => {
    if (err) {
      res.statusCode = 404;
      res.end("not found");
      return;
    }
    const ext = path.extname(full).toLowerCase();
    res.setHeader("Content-Type", MIME[ext] || "application/octet-stream");
    res.end(data);
  });
}

function run() {
  fs.rmSync(OUT_DIR, { recursive: true, force: true });
  fs.mkdirSync(OUT_DIR, { recursive: true });

  const server = http.createServer((req, res) => {
    const reqPath = decodeURIComponent((req.url || "/").split("?")[0]);
    serveFile(reqPath, res);
  });

  server.listen(8080, "127.0.0.1", () => {
    const child = spawn(
      process.execPath,
      [
        CLIENT,
        "--url",
        URL,
        "--actions-file",
        ACTIONS,
        "--iterations",
        "2",
        "--pause-ms",
        "300",
        "--screenshot-dir",
        OUT_DIR,
      ],
      { stdio: "inherit" }
    );

    child.on("exit", (code) => {
      server.close(() => process.exit(code ?? 1));
    });

    child.on("error", () => {
      server.close(() => process.exit(1));
    });
  });
}

run();
