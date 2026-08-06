import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";

export type Priority = "alta" | "media" | "baja";
export type Status = "pendiente" | "completada" | "en-curso" | "vencida";

export interface Category {
  id: string;
  name: string;
  color: string;
  icon: string;
}

export interface Task {
  id: number;
  title: string;
  categoryId: string;
  priority: Priority;
  status: Status;
  start: Date;
  end: Date;
  allDay?: boolean;
  tags?: string[];
  progress?: number;
  description?: string;
  notes?: string;
  links?: string[];
  reminderMinutes?: number | null;
}

interface TaskRow {
  id: number;
  title: string;
  category_id: string;
  priority: string;
  status: string;
  start_at: number;
  end_at: number;
  all_day: boolean;
  progress: number;
  completed_at: number | null;
  created_at: number;
  description: string;
  tags: string;
  notes: string;
  links: string;
  reminder_minutes: number | null;
}

export const categories: Category[] = [
  { id: "uni", name: "Universidad", color: "#2563EB", icon: "graduation-cap" },
  { id: "trab", name: "Trabajo", color: "#7C3AED", icon: "briefcase" },
  { id: "per", name: "Personal", color: "#EC4899", icon: "user" },
  { id: "fin", name: "Finanzas", color: "#F59E0B", icon: "wallet" },
  { id: "sal", name: "Salud", color: "#10B981", icon: "heart-pulse" },
  { id: "otr", name: "Otros", color: "#0EA5E9", icon: "sparkles" },
];

export function cat(id: string): Category {
  return categories.find((c) => c.id === id) ?? categories[categories.length - 1];
}

export interface SyncProgressEvent {
  phase: string;
  mailbox: string;
  processed: number;
  total: number;
}

export interface SyncDoneSummary {
  started_at: number;
  finished_at: number;
  mailboxes: { mailbox: string; found: number; processed: number; result: string; error: string }[];
  total_found: number;
  total_suggestions: number;
  error?: string | null;
}

export interface GeneralSettingsView {
  start_with_windows: boolean;
  start_minimized: boolean;
  close_to_tray_widget: boolean;
  autostart_actual: boolean;
}

const store = $state({
  tasks: [] as Task[],
  ready: false,
  quickadd: 0,
  suggestions: [] as Suggestion[],
  suggestionsPending: 0,
  aiConfig: null as AiConfigView | null,
  emailConfig: null as EmailConfigView | null,
  syncStates: [] as SyncStateRow[],
  syncHistory: [] as SyncHistoryRow[],
  nlBusy: false,
  nlToast: null as null | { text: string; source: string },
  syncRunning: false,
  syncProgress: null as SyncProgressEvent | null,
  syncSummary: null as SyncDoneSummary | null,
  taskDetail: null as Task | null,
  lastRange: null as null | { from: number; to: number },
  general: null as GeneralSettingsView | null,
  syncToday: [] as SyncHistoryRow[],
  lastSyncAt: null as number | null,
  nextSyncAt: null as number | null,
});

export const tasks = () => store.tasks;
export const ready = () => store.ready;
export const quickadd = () => store.quickadd;
export const suggestions = () => store.suggestions;
export const suggestionsPending = () => store.suggestionsPending;
export const aiConfig = () => store.aiConfig;
export const emailConfig = () => store.emailConfig;
export const syncStates = () => store.syncStates;
export const syncHistory = () => store.syncHistory;
export const nlBusy = () => store.nlBusy;
export const nlToast = () => store.nlToast;
export const syncRunning = () => store.syncRunning;
export const syncProgress = () => store.syncProgress;
export const syncSummary = () => store.syncSummary;
export const taskDetail = () => store.taskDetail;
export const lastRange = () => store.lastRange;
export const generalSettings = () => store.general;
export const syncToday = () => store.syncToday;
export const lastSyncAt = () => store.lastSyncAt;
export const nextSyncAt = () => store.nextSyncAt;

export function openTaskDetail(t: Task) {
  store.taskDetail = t;
}
export function closeTaskDetail() {
  store.taskDetail = null;
}

let widgetHFlush: ReturnType<typeof setTimeout> | null = null;
let _widgetHeight = 0;

/** Consulta o actualiza (con debounce) la altura del widget en el backend. */
export function widgetHeight(px?: number): number {
  if (px == null) return _widgetHeight;
  _widgetHeight = Math.max(0, Math.round(px));
  if (!inTauri()) return _widgetHeight;
  if (widgetHFlush) clearTimeout(widgetHFlush);
  widgetHFlush = setTimeout(() => {
    invoke("widget_set_height", { height: _widgetHeight }).catch(() => {});
  }, 60);
  return _widgetHeight;
}

/** Abre la app principal en la vista Agenda (desde el widget). */
export function openAgenda() {
  if (!inTauri()) return;
  invoke("open_agenda").catch(() => {});
}

// ---------- caché de tareas por semana ----------
const weekCache = new Map<string, Map<number, Task>>();

function weekKey(d: Date): string {
  const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const dow = (x.getDay() + 6) % 7;
  x.setDate(x.getDate() - dow);
  x.setHours(0, 0, 0, 0);
  return x.toISOString().slice(0, 10);
}

function rebuildTasks() {
  const map = new Map<number, Task>();
  for (const w of weekCache.values()) {
    for (const [id, t] of w) map.set(id, t);
  }
  store.tasks = [...map.values()].sort((a, b) => a.start.getTime() - b.start.getTime());
}

async function fetchWeek(key: string): Promise<Map<number, Task>> {
  if (weekCache.has(key)) return weekCache.get(key)!;
  const [y, m, d] = key.split("-").map(Number);
  const start = new Date(y, m - 1, d);
  const end = new Date(start.getTime() + 7 * 86_400_000 - 1);
  let rows: TaskRow[] = [];
  if (inTauri()) {
    try {
      rows = await invoke<TaskRow[]>("task_list_range", {
        startAt: start.getTime(),
        endAt: end.getTime(),
      });
    } catch (e) {
      console.error("fetchWeek", key, e);
    }
  } else {
    const base = new Map(demoTasks.map((t) => [t.id, t]));
    weekCache.set(key, base);
    return base;
  }
  const map = new Map<number, Task>();
  for (const r of rows) map.set(r.id, toTask(r));
  weekCache.set(key, map);
  return map;
}

export function weekKeysBetween(from: Date, to: Date): string[] {
  const keys = new Set<string>();
  const cur = weekKey(from);
  const endKey = weekKey(to);
  let d = new Date(cur + "T00:00:00");
  while (cur <= endKey && d.getTime() <= new Date(endKey + "T00:00:00").getTime()) {
    keys.add(weekKey(d));
    d = new Date(d.getTime() + 7 * 86_400_000);
  }
  return [...keys];
}

/** Garantiza que todas las semanas del rango estén cargadas (solo consulta las faltantes). */
export async function ensureRange(from: Date, to: Date) {
  store.lastRange = { from: from.getTime(), to: to.getTime() };
  const missing: string[] = [];
  for (const k of weekKeysBetween(from, to)) {
    if (!weekCache.has(k)) missing.push(k);
  }
  if (missing.length === 0) return;
  await Promise.all(missing.map(fetchWeek));
  rebuildTasks();
}

/** Recarga (fuerza refetch) las semanas del rango visible. */
export async function refreshRange(from: Date, to: Date) {
  for (const k of weekKeysBetween(from, to)) weekCache.delete(k);
  await ensureRange(from, to);
}

/** Inserta/actualiza una tarea en su semana de caché sin recargar todo. */
function putInCache(t: Task) {
  const k = weekKey(t.start);
  if (weekCache.has(k)) {
    weekCache.get(k)!.set(t.id, t);
  }
}

export async function refreshTasks() {
  if (store.lastRange) {
    await refreshRange(new Date(store.lastRange.from), new Date(store.lastRange.to));
  } else {
    await ensureRange(
      new Date(Date.now() - 7 * 86_400_000),
      new Date(Date.now() + 35 * 86_400_000),
    );
  }
}

function bumpQuickadd() {
  store.quickadd += 1;
}

function setNlToast(text: string, source: string) {
  store.nlToast = { text, source };
  setTimeout(() => (store.nlToast = null), 3500);
}

const inTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function toTask(r: TaskRow): Task {
  let status: Status;
  if (r.completed_at != null) status = "completada";
  else if (r.status === "en-curso") status = "en-curso";
  else status = r.start_at < Date.now() ? "vencida" : "pendiente";
  let tags: string[] = [];
  try {
    const parsed = JSON.parse(r.tags || "[]");
    if (Array.isArray(parsed)) tags = parsed.map(String);
  } catch {
    tags = [];
  }
  return {
    id: r.id,
    title: r.title,
    categoryId: r.category_id,
    priority: r.priority as Priority,
    status,
    start: new Date(r.start_at),
    end: new Date(r.end_at),
    allDay: r.all_day,
    progress: r.progress,
    description: r.description,
    tags,
    notes: r.notes,
    links: r.links ? r.links.split(",").map((s) => s.trim()).filter(Boolean) : [],
    reminderMinutes: r.reminder_minutes,
  };
}

function at(daysFromNow: number, h = 0, m = 0): Date {
  const d = new Date();
  d.setDate(d.getDate() + daysFromNow);
  d.setHours(h, m, 0, 0);
  return d;
}

const demoTasks: Task[] = [
  { id: 1, title: "Estudiar cálculo — derivadas e integrales", categoryId: "uni", priority: "alta", status: "en-curso", start: at(0, 9, 0), end: at(0, 11, 0) },
  { id: 2, title: "Entregar proyecto de redes", categoryId: "uni", priority: "alta", status: "vencida", start: at(0, 14, 0), end: at(0, 14, 30) },
  { id: 3, title: "Reunión de proyecto freelance", categoryId: "trab", priority: "media", status: "pendiente", start: at(1, 10, 0), end: at(1, 11, 0) },
  { id: 4, title: "Pagar internet", categoryId: "fin", priority: "media", status: "pendiente", start: at(2, 9, 0), end: at(2, 9, 0) },
  { id: 5, title: "Examen de física — parcial 2", categoryId: "uni", priority: "alta", status: "pendiente", start: at(3, 8, 0), end: at(3, 10, 0) },
  { id: 6, title: "Cita médico — revisión anual", categoryId: "sal", priority: "baja", status: "pendiente", start: at(4, 12, 30), end: at(4, 13, 15) },
  { id: 7, title: "Leer capítulo 4 de teoría de sistemas", categoryId: "uni", priority: "baja", status: "completada", start: at(-1, 16, 0), end: at(-1, 17, 0), progress: 100 },
  { id: 8, title: "Enviar factura de marzo", categoryId: "trab", priority: "media", status: "pendiente", start: at(0, 17, 0), end: at(0, 17, 30) },
  { id: 9, title: "Gimnasio — pierna y glúteo", categoryId: "sal", priority: "baja", status: "pendiente", start: at(0, 18, 30), end: at(0, 19, 30) },
  { id: 10, title: "Dormir temprano para examen", categoryId: "per", priority: "media", status: "pendiente", start: at(2, 22, 0), end: at(2, 23, 0) },
];

export async function loadTasks() {
  if (!inTauri()) {
    store.tasks.length = 0;
    store.tasks.push(...demoTasks);
    store.ready = true;
    return;
  }
  try {
    const rows = await invoke<TaskRow[]>("task_list");
    store.tasks.length = 0;
    store.tasks.push(...rows.map(toTask));
    store.ready = true;
  } catch (e) {
    console.error("loadTasks", e);
  }
}

/** Carga inicial: 6 semanas alrededor de hoy (día −7 hasta +35) para agenda/widget/mes. */
export async function initTasks() {
  if (inTauri()) {
    const from = new Date(Date.now() - 7 * 86_400_000);
    const to = new Date(Date.now() + 35 * 86_400_000);
    await ensureRange(from, to);
    store.ready = true;
  } else {
    await loadTasks();
  }
}

export async function addTask(t: Omit<Task, "id">) {
  if (inTauri()) {
    try {
      const row = await invoke<TaskRow>("task_create", {
        title: t.title,
        categoryId: t.categoryId,
        priority: t.priority,
        startAt: t.start.getTime(),
        endAt: t.end.getTime(),
        allDay: t.allDay ?? false,
      });
      const task = toTask(row);
      putInCache(task);
      rebuildTasks();
    } catch (e) {
      console.error("addTask", e);
    }
    return;
  }
  store.tasks.push({ ...t, id: Math.max(0, ...store.tasks.map((x) => x.id)) + 1 });
}

export async function completeTask(id: number) {
  const t = store.tasks.find((x) => x.id === id);
  if (!t) return;
  const done = t.status !== "completada";
  if (inTauri()) {
    try {
      await invoke("task_complete", { id, done });
    } catch (e) {
      console.error("completeTask", e);
    }
    return;
  }
  t.status = done ? "completada" : "pendiente";
  t.progress = done ? 100 : t.progress;
}

export async function init() {
  await initTasks();
  if (!inTauri()) return;
  try {
    await listen("tasks:changed", () => {
      refreshTasks();
      if (store.taskDetail) {
        // mantén el drawer sincronizado con los datos frescos
        const cur = store.tasks.find((t) => t.id === store.taskDetail!.id);
        if (cur) store.taskDetail = cur;
        else store.taskDetail = null;
      }
    });
    await listen("quickadd", () => bumpQuickadd());
    await listen("email:new-suggestions", () => {
      loadSuggestions();
      refreshTasks();
    });
    await listen("email:sync-done", (e) => {
      store.syncRunning = false;
      store.syncProgress = null;
      store.syncSummary = e.payload as SyncDoneSummary;
      loadSyncStatus();
      loadSuggestions();
      refreshTasks();
    });
    await listen("email:sync-progress", (e) => {
      store.syncRunning = true;
      store.syncProgress = e.payload as SyncProgressEvent;
    });
    await listen("email:sync-error", (e) => {
      store.syncRunning = false;
      store.syncProgress = null;
      store.syncSummary = { started_at: 0, finished_at: 0, mailboxes: [], total_found: 0, total_suggestions: 0, error: String(e.payload) } satisfies SyncDoneSummary;
      loadSyncStatus();
    });
  } catch (e) {
    console.error("init", e);
  }
}

export interface Suggestion {
  id: number;
  source: string;
  source_email_id: string | null;
  source_sender: string | null;
  title: string;
  description: string;
  category_id: string;
  priority: string;
  start_at: number | null;
  end_at: number | null;
  location: string;
  tags: string;
  confidence: number;
  reason: string;
  status: string;
  dedupe_task_id: number | null;
  dedupe_note: string;
  result_task_id: number | null;
  created_at: number;
  updated_at: number;
}

export interface AiConfigView {
  endpoint: string;
  model: string;
  has_key: boolean;
  effective_endpoint: string;
  effective_model: string;
}

export interface EmailConfigView {
  config: {
    host: string;
    port: number;
    user: string;
    auth: string;
    mailboxes: string[];
    filters: { senders: string[]; domains: string[]; keywords: string[] };
    ssl: boolean;
  };
  enabled: boolean;
  interval_hours: number;
  max_age_days: number;
  has_password: boolean;
  trusted: string[];
}

export interface SyncStateRow {
  source: string;
  checkpoint: string;
  last_result: string;
  last_error: string;
  last_run_at: number | null;
}

export interface SyncHistoryRow {
  id: number;
  source: string;
  started_at: number;
  finished_at: number | null;
  result: string;
  items_found: number;
  items_processed: number;
  error: string;
  note: string;
}

export interface TaskFromTextResult {
  task: {
    id: number;
    title: string;
    category_id: string;
    priority: string;
    status: string;
    start_at: number;
    end_at: number;
    all_day: boolean;
    progress: number;
    completed_at: number | null;
    created_at: number;
  };
  source: string;
  used_ai: boolean;
}

export async function loadSuggestions() {
  if (!inTauri()) {
    store.suggestions = [];
    store.suggestionsPending = 0;
    return;
  }
  try {
    const rows = await invoke<Suggestion[]>("suggestions_list", { onlyPending: false });
    store.suggestions = rows;
    store.suggestionsPending = rows.filter((s) => s.status === "pending").length;
  } catch (e) {
    console.error("loadSuggestions", e);
  }
}

export async function loadAiConfig() {
  if (!inTauri()) return;
  try {
    store.aiConfig = await invoke<AiConfigView>("ai_config_get");
  } catch (e) {
    console.error("loadAiConfig", e);
  }
}

export async function loadEmailConfig() {
  if (!inTauri()) return;
  try {
    store.emailConfig = await invoke<EmailConfigView>("email_config_get");
  } catch (e) {
    console.error("loadEmailConfig", e);
  }
}

export async function loadSyncStatus() {
  if (!inTauri()) return;
  try {
    const v = await invoke<{
      states: SyncStateRow[];
      today: SyncHistoryRow[];
      last_history: SyncHistoryRow[];
      last_sync_at: number | null;
      next_sync_at: number | null;
      interval_hours: number;
    }>("sync_status");
    store.syncStates = v.states;
    store.syncHistory = v.last_history;
    store.syncToday = v.today;
    store.lastSyncAt = v.last_sync_at;
    store.nextSyncAt = v.next_sync_at;
  } catch (e) {
    console.error("loadSyncStatus", e);
  }
}

export async function loadGeneralSettings() {
  if (!inTauri()) return;
  try {
    store.general = await invoke<GeneralSettingsView>("general_settings_get");
  } catch (e) {
    console.error("loadGeneralSettings", e);
  }
}

export async function saveGeneralSettings(v: {
  startWithWindows: boolean;
  startMinimized: boolean;
  closeToTrayWidget: boolean;
}): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    await invoke("general_settings_set", {
      startWithWindows: v.startWithWindows,
      startMinimized: v.startMinimized,
      closeToTrayWidget: v.closeToTrayWidget,
    });
    store.general = { ...v, autostart_actual: v.startWithWindows };
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

export async function createTaskFromText(text: string): Promise<{ ok: boolean; source: string; error?: string }> {
  if (!inTauri()) {
    store.tasks.push({
      id: Math.max(0, ...store.tasks.map((x) => x.id)) + 1,
      title: text,
      categoryId: "otr",
      priority: "media",
      status: "pendiente",
      start: new Date(),
      end: new Date(),
    });
    return { ok: true, source: "local" };
  }
  store.nlBusy = true;
  try {
    const r = await invoke<TaskFromTextResult>("task_from_text", { text });
    putInCache(toTask(r.task));
    rebuildTasks();
    return { ok: true, source: r.source };
  } catch (e) {
    return { ok: false, source: "error", error: String(e) };
  } finally {
    store.nlBusy = false;
  }
}

export interface TaskEditData {
  title: string;
  description: string;
  categoryId: string;
  priority: string;
  startAt: number;
  endAt: number;
  tags: string[];
  notes: string;
  links: string[];
  reminderMinutes: number | null;
}

/** Guarda la edición completa de una tarea y sincroniza caché/drawer al instante. */
export async function updateTaskDetail(id: number, data: TaskEditData): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    await invoke("task_update", {
      id,
      title: data.title,
      categoryId: data.categoryId,
      priority: data.priority,
      startAt: Math.round(data.startAt),
      endAt: Math.round(data.endAt),
      description: data.description,
      tags: JSON.stringify(data.tags),
      notes: data.notes,
      links: data.links.join(", "),
      reminderMinutes: data.reminderMinutes,
    });
    const cur = store.tasks.find((t) => t.id === id);
    if (cur) {
      const updated: Task = {
        ...cur,
        title: data.title,
        categoryId: data.categoryId,
        priority: data.priority as Priority,
        start: new Date(data.startAt),
        end: new Date(data.endAt),
        description: data.description,
        tags: data.tags,
        notes: data.notes,
        links: data.links,
        reminderMinutes: data.reminderMinutes,
      };
      const oldKey = weekKey(cur.start);
      weekCache.get(oldKey)?.delete(id);
      putInCache(updated);
      rebuildTasks();
      store.taskDetail = updated;
    } else {
      await refreshTasks();
    }
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

/** Mueve/redimensiona una tarea (drag & drop). Valida conflictos en el backend. */
export async function moveTask(
  id: number,
  startAt: number,
  endAt: number,
): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    await invoke("task_move", { id, startAt: Math.round(startAt), endAt: Math.round(endAt) });
    const cur = store.tasks.find((t) => t.id === id);
    if (cur) {
      const moved: Task = { ...cur, start: new Date(startAt), end: new Date(endAt) };
      const oldKey = weekKey(cur.start);
      weekCache.get(oldKey)?.delete(id);
      putInCache(moved);
      rebuildTasks();
      if (store.taskDetail?.id === id) store.taskDetail = moved;
    }
    return { ok: true };
  } catch (e) {
    const msg = String(e);
    const m = msg.match(/conflicto: se solapa con '(.*)'/);
    return { ok: false, error: m ? m[1] : msg };
  }
}

export async function duplicateTask(id: number): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    const row = await invoke<TaskRow>("task_duplicate", { id });
    putInCache(toTask(row));
    rebuildTasks();
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

export async function deleteTask(id: number): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    await invoke("task_delete", { id });
    for (const w of weekCache.values()) w.delete(id);
    rebuildTasks();
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

export async function suggestionAccept(id: number) {
  if (!inTauri()) return;
  try {
    await invoke("suggestion_accept", { id });
    await refreshTasks();
    await loadSuggestions();
  } catch (e) {
    console.error("suggestionAccept", e);
  }
}

export async function suggestionReject(id: number) {
  if (!inTauri()) return;
  try {
    await invoke("suggestion_reject", { id });
    await loadSuggestions();
  } catch (e) {
    console.error("suggestionReject", e);
  }
}

export async function suggestionEdit(
  id: number,
  data: { title: string; categoryId: string; priority: string; startAt: number; endAt: number; description: string },
) {
  if (!inTauri()) return;
  try {
    await invoke("suggestion_edit", { id, ...data });
    await loadSuggestions();
  } catch (e) {
    console.error("suggestionEdit", e);
  }
}

export async function suggestionMerge(id: number, taskId: number) {
  if (!inTauri()) return;
  try {
    await invoke("suggestion_merge", { id, taskId });
    await refreshTasks();
    await loadSuggestions();
  } catch (e) {
    console.error("suggestionMerge", e);
  }
}

export async function suggestionRevert(id: number) {
  if (!inTauri()) return;
  try {
    await invoke("suggestion_revert", { id });
    await refreshTasks();
    await loadSuggestions();
  } catch (e) {
    console.error("suggestionRevert", e);
  }
}

export async function syncNow() {
  if (!inTauri()) return;
  try {
    store.syncRunning = true;
    store.syncProgress = null;
    store.syncSummary = null;
    await invoke("email_sync_now");
    setNlToast("Sincronizando correo…", "sync");
  } catch (e) {
    store.syncRunning = false;
    console.error("syncNow", e);
  }
}

export function fmtMs(ms: number | null): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return d.toLocaleDateString("es-ES", { day: "2-digit", month: "short" }) +
    " " + d.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
}

export function fmtDate(ms: number | null): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleDateString("es-ES", {
    weekday: "short",
    day: "2-digit",
    month: "short",
  });
}

export function toggleTheme() {
  const root = document.documentElement;
  const next = root.dataset.theme === "dark" ? "light" : "dark";
  root.dataset.theme = next;
  try {
    localStorage.setItem("ff-theme", next);
  } catch {
    // sin almacenamiento (navegador restringido)
  }
  if (inTauri()) {
    emit("theme:changed", next).catch(() => {});
  }
}

/** Aplica el tema guardado (se llama en cada ventana al montar). */
export function applySavedTheme() {
  try {
    const saved = localStorage.getItem("ff-theme");
    if (saved === "dark" || saved === "light") {
      document.documentElement.dataset.theme = saved;
      return saved;
    }
  } catch {
    // ignorar
  }
  return document.documentElement.dataset.theme === "dark" ? "dark" : "light";
}

export const MONTHS_ES = [
  "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio",
  "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre",
];
export const DAYS_ES = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];
