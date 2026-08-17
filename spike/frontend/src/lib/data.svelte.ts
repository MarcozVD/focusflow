import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
  conflict_strict: boolean;
  autostart_actual: boolean;
}

// ---------------- propuestas de planificación (fase 7) ----------------

export interface PlanSessionView {
  start_ms: number;
  end_ms: number;
  is_prep: boolean;
}

export interface PlanItemView {
  title: string;
  intent_type: string;
  priority: string;
  category_id: string;
  deadline_bound_ms: number | null;
  prep_min: number;
  task_min: number;
  required_min: number;
  planned_min: number;
  complete: boolean;
  notes: string[];
  sessions: PlanSessionView[];
}

export interface UnderstoodView {
  title: string;
  intent_type: string;
  category_id: string;
  priority: string;
  when_label: string;
  deadline: number | null;
  window_start: number | null;
  window_end: number | null;
  all_day: boolean;
  prep_min: number;
  task_min: number;
  reminders_min_before: number[];
}

export interface PlanProposalView {
  id: number;
  text: string;
  status: string;
  source: string;
  understanding: UnderstoodView[];
  items: PlanItemView[];
  created_at: number;
}

export interface EditedPlan {
  items: { start_ms: number; end_ms: number }[][];
}

export interface HistoryMsg {
  role: "user" | "assistant";
  text: string;
}

export interface AssistantActionView {
  proposal_id: number;
  kind: string;
  task_title: string;
  title: string;
  category_id: string;
  priority: string;
  start_ms: number | null;
  end_ms: number | null;
  all_day: boolean;
  summary: string;
}

export type TaskRefView = {
  id: number;
  title: string;
  cat: string;
  priority: string;
  start_ms: number;
  end_ms: number;
  all_day: boolean;
  level: "URGENT" | "IMPORTANT" | "NORMAL";
};

export type AssistantTurn =
  | { type: "Answer"; text: string; tasks?: TaskRefView[] }
  | { type: "Plan"; proposal: PlanProposalView; note: string }
  | { type: "Action"; action: AssistantActionView }
  | { type: "Nothing"; text: string };

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
  theme: "" as "" | "light" | "dark",
  accent: "#2563EB",
  planProposal: null as PlanProposalView | null,
  // Resultado persistente por propuesta de plan: sobrevive al re-montaje de
  // pestañas. El Asistente lo lee para no volver a mostrar los botones de
  // Aceptar/Descartar de una propuesta ya resuelta.
  planResults: {} as Record<number, { ok: boolean; text: string }>,
  planBusy: false,
  planError: "",
  assistantThread: [] as { role: "user" | "assistant"; turn: AssistantTurn | null; text: string; at: number }[],
  assistantBusy: false,
  assistantError: "",
  // Última petición que falló por 429; re-enviable con el botón Reintentar.
  assistantRetry: "",
  assistantActions: 0,
  notifPrefs: null as NotifPrefsView | null,
  contextualNotif: null as ContextualNotif | null,
  assistantDraft: "",
  onboarding: null as OnboardingStatus | null,
  onboardingBusy: false,
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
export const uiTheme = () => store.theme;
export const uiAccent = () => store.accent;
export const planProposal = () => store.planProposal;
/** Resultado persistente de una propuesta de plan (aceptada/rechazada/error). */
export const planResult = (id: number) => store.planResults[id];
export const planBusy = () => store.planBusy;
export const planError = () => store.planError;
export const assistantThread = () => store.assistantThread;
export const assistantBusy = () => store.assistantBusy;
export const assistantError = () => store.assistantError;
/** Petición pendiente de reintento por 429 ("" = no hay). */
export const assistantRetry = () => store.assistantRetry;
export const assistantActionsPending = () => store.assistantActions;
export const onboarding = () => store.onboarding;
export const onboardingBusy = () => store.onboardingBusy;

export function closePlanProposal() {
  store.planProposal = null;
  store.planError = "";
}

/** Texto → propuesta de plan (no toca el calendario hasta aceptar).
 *  `opts.local` usa el parser de reglas (instantáneo, sin IA). La última
 *  petición gana: si el usuario pide la vía local mientras la IA sigue
 *  pensando, la respuesta tardía de la IA se descarta. */
let planReqId = 0;

export async function planFromText(text: string, opts?: { local?: boolean }): Promise<{ ok: boolean; source: string; error?: string }> {
  if (!inTauri()) {
    store.planProposal = null;
    return { ok: false, source: "stale", error: "sin Tauri" };
  }
  const myId = ++planReqId;
  store.planBusy = true;
  store.planError = "";
  try {
    const view = opts?.local
      ? await invoke<PlanProposalView>("plan_from_text_local", { text })
      : await invoke<PlanProposalView>("plan_from_text", { text });
    if (myId !== planReqId) return { ok: false, source: "stale", error: "reemplazada" };
    store.planProposal = view;
    return { ok: true, source: view.source };
  } catch (e) {
    if (myId !== planReqId) return { ok: false, source: "stale", error: "reemplazada" };
    store.planError = String(e);
    return { ok: false, source: "error", error: String(e) };
  } finally {
    if (myId === planReqId) store.planBusy = false;
  }
}

/** Acepta la propuesta; `edit` (opcional) reemplaza los bloques por ítem. */
export async function planAccept(id: number, edit?: EditedPlan): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) {
    // demo: simular aceptación y cerrar la propuesta
    store.planProposal = null;
    store.planResults[id] = { ok: true, text: "Propuesta aceptada y añadida al calendario." };
    return { ok: true };
  }
  store.planBusy = true;
  store.planError = "";
  try {
    await invoke("plan_accept", { id, edit: edit ?? null });
    store.planProposal = null;
    store.planResults[id] = { ok: true, text: "Propuesta aceptada y añadida al calendario." };
    await refreshTasks();
    return { ok: true };
  } catch (e) {
    store.planError = String(e);
    store.planResults[id] = { ok: false, text: String(e) };
    return { ok: false, error: String(e) };
  } finally {
    store.planBusy = false;
  }
}

/** Cancela la propuesta sin cambios en el calendario. */
export async function planReject(id: number): Promise<void> {
  if (!inTauri()) {
    store.planProposal = null;
    store.planResults[id] = { ok: true, text: "Propuesta cancelada." };
    return;
  }
  try {
    await invoke("plan_reject", { id });
  } catch (e) {
    console.error("planReject", e);
  }
  store.planProposal = null;
  store.planError = "";
  store.planResults[id] = { ok: true, text: "Propuesta cancelada." };
}

const MAX_HISTORY_TURNS = 6;

function historyForTurn(): HistoryMsg[] {
  return store.assistantThread
    .filter((m) => m.role === "user" || (m.turn && m.turn.type !== "Action"))
    .slice(-MAX_HISTORY_TURNS)
    .map((m) => {
      if (m.role === "user") return { role: "user", text: m.text };
      const t = m.turn;
      const text =
        t?.type === "Answer"
          ? t.text
          : t?.type === "Nothing"
            ? t.text
            : t?.type === "Plan"
              ? t.note
              : t?.type === "Action"
                ? `(acción propuesta: ${t.action.summary})`
                : m.text;
      return { role: "assistant", text };
    });
}

/** Envía un turno al asistente. La respuesta se añade al hilo. */
export async function assistantTurn(text: string): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: false, error: "sin Tauri" };
  store.assistantBusy = true;
  store.assistantError = "";
  store.assistantRetry = "";
  store.assistantThread.push({ role: "user", turn: null, text, at: Date.now() });
  try {
    const turn = await invoke<AssistantTurn>("assistant_turn", { text, history: historyForTurn() });
    store.assistantThread.push({ role: "assistant", turn, text: "", at: Date.now() });
    if (turn.type === "Plan") store.planProposal = turn.proposal;
    if (turn.type === "Action") {
      store.assistantActions++;
      refreshAssistantActions();
    }
    await refreshTasks();
    return { ok: true };
  } catch (e) {
    const friendly = friendlyAssistantError(e);
    store.assistantError = friendly.text;
    if (friendly.retryable) store.assistantRetry = text;
    store.assistantThread.push({
      role: "assistant",
      turn: { type: "Nothing", text: friendly.text },
      text: "",
      at: Date.now(),
    });
    return { ok: false, error: friendly.text };
  } finally {
    store.assistantBusy = false;
  }
}

export async function assistantActionAccept(id: number): Promise<string> {
  const r = await invoke<string>("assistant_action_accept", { id });
  store.assistantActions = Math.max(0, store.assistantActions - 1);
  await refreshTasks();
  await loadSuggestions();
  return r;
}

export async function assistantActionReject(id: number): Promise<void> {
  await invoke("assistant_action_reject", { id });
  store.assistantActions = Math.max(0, store.assistantActions - 1);
}

export async function refreshAssistantActions() {
  try {
    const rows = await invoke<{ status: string }[]>("assistant_actions_list", { onlyPending: true });
    store.assistantActions = rows.length;
  } catch {
    /* noop */
  }
}

// ---------------- notificaciones contextuales (fase 11) ----------------

export interface NotifPrefsView {
  enabled: boolean;
  quiet_start: string;
  quiet_end: string;
  daily_cap: number;
  free_minutes: number;
}

export interface ContextualNotif {
  log_id: number;
  kind: "deadline" | "missed" | "conflict" | "free_time" | "important" | "reschedule";
  task_id: number;
  title: string;
  body: string;
  task_title: string;
}

export const notifPrefs = () => store.notifPrefs;
export const contextualNotif = () => store.contextualNotif;

export async function loadNotifPrefs() {
  if (!inTauri()) return;
  try {
    store.notifPrefs = await invoke<NotifPrefsView>("notif_prefs_get");
  } catch (e) {
    console.error("loadNotifPrefs", e);
  }
}

export async function saveNotifPrefs(p: NotifPrefsView): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: false, error: "sin Tauri" };
  try {
    await invoke("notif_prefs_set", {
      enabled: p.enabled,
      quietStart: p.quiet_start,
      quietEnd: p.quiet_end,
      dailyCap: p.daily_cap,
      freeMinutes: p.free_minutes,
    });
    store.notifPrefs = p;
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

/** Respuesta del usuario a una notificación contextual (para el registro anti-spam). */
export async function notifRespond(id: number, status: "planned" | "later" | "dismissed") {
  if (!inTauri()) return;
  try {
    await invoke("notif_respond", { id, status });
  } catch {
    /* noop */
  }
}

export function closeContextualNotif() {
  store.contextualNotif = null;
}

/** Prompt prefabricado para el asistente (desde notificaciones contextuales). */
export function setAssistantDraft(text: string) {
  store.assistantDraft = text;
}
export function takeAssistantDraft(): string {
  const t = store.assistantDraft;
  store.assistantDraft = "";
  return t;
}

export { friendlyAssistantError } from "./assistantError";
import type { FriendlyAssistantError } from "./assistantError";
export type { FriendlyAssistantError };

export function clearAssistantThread() {
  store.assistantThread = [];
  store.assistantError = "";
  store.assistantRetry = "";
}

export function openTaskDetail(t: Task) {
  store.taskDetail = t;
}
export function closeTaskDetail() {
  store.taskDetail = null;
}

/** Abre la app principal en la vista Agenda (desde el widget). */
export function openAgenda() {
  if (!inTauri()) return;
  invoke("open_agenda").catch(() => {});
}

/** Abre la app principal (desde el widget). */
export function openApp() {
  if (!inTauri()) return;
  invoke("open_app").catch(() => {});
}

/** Abre la app en el Asistente (desde el widget). */
export function askAssistant() {
  if (!inTauri()) return;
  invoke("open_assistant").catch(() => {});
}

/** Acción rápida del widget: complete | postpone | start (vía servicios). */
export async function widgetAction(id: number, action: "complete" | "postpone" | "start") {
  if (!inTauri()) return;
  try {
    await invoke("widget_action", { id, action });
  } catch (e) {
    console.error("widgetAction", e);
  }
}

/** Abre una tarea directamente en la app principal (desde el widget). */
export function openTaskRemote(id: number) {
  if (!inTauri()) return;
  invoke("open_task", { id }).catch(() => {});
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
const MAX_CACHED_WEEKS = 64;

export async function ensureRange(from: Date, to: Date) {
  store.lastRange = { from: from.getTime(), to: to.getTime() };
  const missing: string[] = [];
  for (const k of weekKeysBetween(from, to)) {
    if (!weekCache.has(k)) missing.push(k);
  }
  if (missing.length > 0) {
    await Promise.all(missing.map(fetchWeek));
    rebuildTasks();
  }
  // memoria acotada: si el caché crece, se evictan las semanas fuera del rango visible
  if (weekCache.size > MAX_CACHED_WEEKS) {
    const visible = new Set(weekKeysBetween(from, to));
    for (const k of [...weekCache.keys()]) {
      if (weekCache.size <= MAX_CACHED_WEEKS) break;
      if (!visible.has(k)) weekCache.delete(k);
    }
    rebuildTasks();
  }
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

/** El widget y la ventana principal montan App + WidgetPage: los listeners
 *  de eventos solo se registran una vez por proceso. */
let listenersReady = false;

export async function init() {
  await initTasks();
  if (!inTauri()) return;
  if (listenersReady) return;
  listenersReady = true;
  try {
    refreshAssistantActions();
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
    await listen("notif:contextual", (e) => {
      store.contextualNotif = e.payload as ContextualNotif;
    });
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
  source_subject: string;
  kind: string;
  title: string;
  description: string;
  category_id: string;
  priority: string;
  start_at: number | null;
  end_at: number | null;
  deadline_at: number | null;
  prep_min: number;
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

export const KIND_LABELS: Record<string, string> = {
  event: "Evento",
  deadline: "Vencimiento",
  availability: "Disponibilidad",
  task: "Tarea",
};

export interface AiConfigView {
  endpoint: string;
  model: string;
  has_key: boolean;
  effective_endpoint: string;
  effective_model: string;
}

export interface OnboardingStatus {
  completed: boolean;
  ai: {
    endpoint: string;
    model: string;
    effective_endpoint: string;
    effective_model: string;
    has_key: boolean;
  };
  email: {
    host: string;
    port: number;
    user: string;
    auth: string;
    mailboxes: string[];
    filters: { senders: string[]; domains: string[]; keywords: string[] };
    ssl: boolean;
  } | null;
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

export interface TaskMoveResult {
  conflict: string | null;
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

export async function loadOnboardingStatus() {
  if (!inTauri()) return;
  try {
    store.onboarding = await invoke<OnboardingStatus>("onboarding_status");
  } catch (e) {
    console.error("loadOnboardingStatus", e);
  }
}

export async function completeOnboarding(): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  store.onboardingBusy = true;
  try {
    await invoke("onboarding_complete");
    if (store.onboarding) store.onboarding = { ...store.onboarding, completed: true };
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  } finally {
    store.onboardingBusy = false;
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
  conflictStrict: boolean;
}): Promise<{ ok: boolean; error?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    await invoke("general_settings_set", {
      startWithWindows: v.startWithWindows,
      startMinimized: v.startMinimized,
      closeToTrayWidget: v.closeToTrayWidget,
      conflictStrict: v.conflictStrict,
    });
    store.general = { ...v, autostart_actual: v.startWithWindows };
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e) };
  }
}

/** Contador de solicitudes de lenguaje natural: la última solicitud gana
 *  y las respuestas antiguas se descartan (cancelación de la pendiente). */
let nlReqId = 0;

export async function createTaskFromText(text: string): Promise<{ ok: boolean; source: string; error?: string }> {
  const myId = ++nlReqId;
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
  // una creación a la vez: la tarea se crea en backend antes de cualquier
  // chequeo; sin esta guardia, Enter repetido la duplicaba
  if (store.nlBusy) return { ok: false, source: "stale", error: "creación en curso" };
  store.nlBusy = true;
  try {
    const r = await invoke<TaskFromTextResult>("task_from_text", { text });
    if (myId !== nlReqId) return { ok: false, source: "stale", error: "cancelada" };
    putInCache(toTask(r.task));
    rebuildTasks();
    return { ok: true, source: r.source };
  } catch (e) {
    if (myId !== nlReqId) return { ok: false, source: "stale", error: "cancelada" };
    return { ok: false, source: "error", error: String(e) };
  } finally {
    if (myId === nlReqId) store.nlBusy = false;
  }
}

export interface TaskEditData {
  title: string;
  description: string;
  categoryId: string;
  priority: string;
  startAt: number;
  endAt: number;
  allDay: boolean;
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
      allDay: data.allDay,
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
        allDay: data.allDay,
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

/** Mueve/redimensiona una tarea (drag & drop). El backend valida conflictos
 *  de horario: por defecto avisa sin impedir; en modo estricto devuelve error. */
export async function moveTask(
  id: number,
  startAt: number,
  endAt: number,
  allDay?: boolean,
): Promise<{ ok: boolean; error?: string; conflict?: string }> {
  if (!inTauri()) return { ok: true };
  try {
    const r = await invoke<TaskMoveResult>("task_move", {
      id,
      startAt: Math.round(startAt),
      endAt: Math.round(endAt),
      allDay: allDay ?? null,
    });
    const cur = store.tasks.find((t) => t.id === id);
    if (cur) {
      const moved: Task = { ...cur, start: new Date(startAt), end: new Date(endAt) };
      if (allDay !== undefined) moved.allDay = allDay;
      const oldKey = weekKey(cur.start);
      weekCache.get(oldKey)?.delete(id);
      putInCache(moved);
      rebuildTasks();
      if (store.taskDetail?.id === id) store.taskDetail = moved;
    }
    return { ok: true, conflict: r.conflict ?? undefined };
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
  data: { title: string; categoryId: string; priority: string; startAt: number; endAt: number; description: string; allDay: boolean },
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

export async function suggestionDelete(id: number) {
  if (!inTauri()) return;
  try {
    await invoke("suggestion_delete", { id });
    await refreshTasks();
    await loadSuggestions();
  } catch (e) {
    console.error("suggestionDelete", e);
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

/** Reescanear la ventana reciente: recupera correos excluidos por filtros. */
export async function rescanEmail() {
  if (!inTauri()) return;
  try {
    store.syncRunning = true;
    store.syncProgress = null;
    store.syncSummary = null;
    await invoke("email_rescan");
    setNlToast("Reescanenado el correo…", "sync");
  } catch (e) {
    store.syncRunning = false;
    console.error("rescanEmail", e);
  }
}

export function fmtMs(ms: number | null): string {
  if (!ms) return "—";
  const d = new Date(ms);
  return d.toLocaleDateString("es-ES", { day: "2-digit", month: "short" }) +
    " " + d.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
}

/** Exporta los datos del usuario (JSON, sin secretos) y los descarga. */
export async function exportData() {
  if (!inTauri()) return;
  try {
    const json = await invoke<string>("data_export");
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `focusflow-export-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    return true;
  } catch (e) {
    console.error("exportData", e);
    return false;
  }
}

/** Borra todos los datos (DB, log y credenciales del sistema). Irreversible. */
export async function wipeData() {
  if (!inTauri()) return;
  try {
    await invoke("data_wipe", { confirmation: "WIPE" });
    window.location.reload();
  } catch (e) {
    console.error("wipeData", e);
    throw e;
  }
}

export function fmtDate(ms: number | null): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleDateString("es-ES", {
    weekday: "short",
    day: "2-digit",
    month: "short",
  });
}

export interface UiPrefs {
  theme: string;
  accent: string;
}

/** Aplica tema + acento al documento y los guarda en localStorage (puente entre ventanas). */
export function applyUiPrefs(p: { theme?: string; accent?: string }) {
  if (p.theme === "dark" || p.theme === "light") {
    document.documentElement.dataset.theme = p.theme;
    store.theme = p.theme;
  }
  if (p.accent && /^#[0-9a-fA-F]{6}$/.test(p.accent)) {
    document.documentElement.style.setProperty("--accent", p.accent);
    store.accent = p.accent;
  }
  try {
    localStorage.setItem(
      "ff-ui",
      JSON.stringify({
        theme: p.theme ?? (store.theme || "light"),
        accent: p.accent ?? store.accent,
      }),
    );
  } catch {
    // sin almacenamiento (navegador restringido)
  }
}

/** Carga prefs persistidas (backend como fuente de verdad; localStorage como fast path). */
export async function loadUiPrefs() {
  if (inTauri()) {
    try {
      const v = await invoke<{ theme: string; accent: string }>("ui_prefs_get");
      applyUiPrefs({ theme: v.theme || undefined, accent: v.accent });
      return;
    } catch (e) {
      console.error("loadUiPrefs", e);
    }
  }
  try {
    const raw = localStorage.getItem("ff-ui");
    if (raw) {
      const p = JSON.parse(raw);
      applyUiPrefs(p);
    }
  } catch {
    // ignorar
  }
}

/** Persiste tema/acento en backend y lo difunde a todas las ventanas (widget incluido). */
export async function setUiPrefs(p: { theme?: string; accent?: string }) {
  const theme = p.theme ?? store.theme;
  const accent = p.accent ?? store.accent;
  applyUiPrefs({ theme, accent });
  if (!inTauri()) return;
  try {
    await invoke("ui_prefs_set", { theme, accent });
  } catch (e) {
    console.error("setUiPrefs", e);
  }
}

export function toggleTheme() {
  const next = store.theme === "dark" ? "light" : "dark";
  setUiPrefs({ theme: next });
}

/** Aplica el tema/acento guardados (se llama en cada ventana al montar, antes del backend).
 *  Devuelve el tema resuelto: el selector y la UI derivan siempre del mismo estado. */
export function applySavedTheme(): "" | "light" | "dark" {
  let theme: "" | "light" | "dark" = "";
  try {
    const raw = localStorage.getItem("ff-ui");
    if (raw) {
      const p = JSON.parse(raw);
      if (p.theme === "dark" || p.theme === "light") theme = p.theme;
      if (p.accent && /^#[0-9a-fA-F]{6}$/.test(p.accent)) {
        document.documentElement.style.setProperty("--accent", p.accent);
        store.accent = p.accent;
      }
    }
  } catch {
    // ignorar
  }
  if (!theme) {
    try {
      const saved = localStorage.getItem("ff-theme");
      if (saved === "dark" || saved === "light") theme = saved;
    } catch {
      // ignorar
    }
  }
  if (theme) {
    document.documentElement.dataset.theme = theme;
    store.theme = theme;
  }
  return theme;
}

export const MONTHS_ES = [
  "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio",
  "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre",
];
export const DAYS_ES = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];
