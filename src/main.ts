import { invoke } from "@tauri-apps/api/core";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

// A global shortcut works while another app is in front.
const SHORTCUT = "CommandOrControl+Shift+R";
const SHORTCUT_LABEL = navigator.userAgent.includes("Mac") ? "⌘⇧R" : "Ctrl+Shift+R";

const POINTER_POLL_MS = 50;

const appWindow = getCurrentWindow();
const frame = document.querySelector<HTMLElement>("#frame")!;
const bar = document.querySelector<HTMLElement>("#bar")!;
const toggleButton = document.querySelector<HTMLButtonElement>("#toggle")!;
const statusButton = document.querySelector<HTMLButtonElement>("#status")!;
const closeButton = document.querySelector<HTMLButtonElement>("#close")!;

let recording: { path: string; startedAt: number; timer: number; pointerTimer: number } | null = null;
let savedPath: string | null = null;

const pad = (value: number) => String(value).padStart(2, "0");

function fileName() {
  const now = new Date();
  const date = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `capup-${date}-${time}.mp4`;
}

function showIdle(status: string) {
  delete frame.dataset.recording;
  toggleButton.textContent = `● REC ${SHORTCUT_LABEL}`;
  statusButton.textContent = status;
}

function showElapsed(startedAt: number) {
  const seconds = Math.floor((Date.now() - startedAt) / 1000);
  statusButton.textContent = `${pad(Math.floor(seconds / 60))}:${pad(seconds % 60)}`;
}

/**
 * Keeps the control bar clickable while the rest of the window lets clicks through to the apps behind it.
 * The window gets no mouse events while it lets clicks through, so the cursor is polled instead.
 */
async function passClicksExceptBar() {
  const [origin, scale] = await Promise.all([appWindow.innerPosition(), appWindow.scaleFactor()]);
  let ignoring = true;
  return setInterval(async () => {
    const cursor = await cursorPosition();
    if (!recording) return;
    const x = (cursor.x - origin.x) / scale;
    const y = (cursor.y - origin.y) / scale;
    const rect = bar.getBoundingClientRect();
    const overBar = x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
    if (ignoring !== overBar) return;
    ignoring = !overBar;
    await appWindow.setIgnoreCursorEvents(ignoring);
  }, POINTER_POLL_MS);
}

async function start() {
  savedPath = null;
  const path = await invoke<string>("start_recording", { fileName: fileName() });
  const startedAt = Date.now();
  recording = {
    path,
    startedAt,
    timer: setInterval(() => showElapsed(startedAt), 500),
    pointerTimer: await passClicksExceptBar(),
  };
  frame.dataset.recording = "";
  toggleButton.textContent = `■ STOP ${SHORTCUT_LABEL}`;
  showElapsed(startedAt);
}

async function stop(current: NonNullable<typeof recording>) {
  recording = null;
  clearInterval(current.timer);
  clearInterval(current.pointerTimer);
  await invoke("stop_recording");
  savedPath = current.path;
  showIdle(`Saved: ${current.path}`);
}

async function toggle() {
  try {
    await (recording ? stop(recording) : start());
  } catch (error) {
    showIdle(String(error));
  }
}

toggleButton.addEventListener("click", toggle);
statusButton.addEventListener("click", () => {
  if (savedPath) revealItemInDir(savedPath);
});
closeButton.addEventListener("click", () => appWindow.close());
register(SHORTCUT, (event) => {
  if (event.state === "Pressed") toggle();
});

showIdle("Drag to move, then record what is behind");
