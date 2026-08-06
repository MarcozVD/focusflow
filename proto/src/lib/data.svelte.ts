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

function at(daysFromNow: number, h = 0, m = 0): Date {
  const d = new Date();
  d.setDate(d.getDate() + daysFromNow);
  d.setHours(h, m, 0, 0);
  return d;
}

export let tasks = $state<Task[]>([
  {
    id: 1, title: "Estudiar cálculo — derivadas e integrales", categoryId: "uni",
    priority: "alta", status: "en-curso",
    start: at(0, 9, 0), end: at(0, 11, 0),
  },
  {
    id: 2, title: "Entregar proyecto de redes", categoryId: "uni",
    priority: "alta", status: "vencida",
    start: at(0, 14, 0), end: at(0, 14, 30),
  },
  {
    id: 3, title: "Reunión de proyecto freelance", categoryId: "trab",
    priority: "media", status: "pendiente",
    start: at(1, 10, 0), end: at(1, 11, 0),
  },
  {
    id: 4, title: "Pagar internet", categoryId: "fin",
    priority: "media", status: "pendiente",
    start: at(2, 9, 0), end: at(2, 9, 0),
  },
  {
    id: 5, title: "Examen de física — parcial 2", categoryId: "uni",
    priority: "alta", status: "pendiente",
    start: at(3, 8, 0), end: at(3, 10, 0),
  },
  {
    id: 6, title: "Cita médico — revisión anual", categoryId: "sal",
    priority: "baja", status: "pendiente",
    start: at(4, 12, 30), end: at(4, 13, 15),
  },
  {
    id: 7, title: "Leer capítulo 4 de teoría de sistemas", categoryId: "uni",
    priority: "baja", status: "completada",
    start: at(-1, 16, 0), end: at(-1, 17, 0), progress: 100,
  },
  {
    id: 8, title: "Enviar factura de marzo", categoryId: "trab",
    priority: "media", status: "pendiente",
    start: at(0, 17, 0), end: at(0, 17, 30),
  },
  {
    id: 9, title: "Gimnasio — pierna y glúteo", categoryId: "sal",
    priority: "baja", status: "pendiente",
    start: at(0, 18, 30), end: at(0, 19, 30),
  },
  {
    id: 10, title: "Dormir temprano para examen", categoryId: "per",
    priority: "media", status: "pendiente",
    start: at(2, 22, 0), end: at(2, 23, 0),
  },
]);

export function addTask(t: Omit<Task, "id">) {
  tasks.push({ ...t, id: Math.max(0, ...tasks.map((x) => x.id)) + 1 });
}

export function completeTask(id: number) {
  const t = tasks.find((x) => x.id === id);
  if (!t) return;
  const done = t.status === "completada";
  t.status = done ? "pendiente" : "completada";
  t.progress = done ? t.progress : 100;
}

export function toggleTheme() {
  const root = document.documentElement;
  const next = root.dataset.theme === "dark" ? "light" : "dark";
  root.dataset.theme = next;
}

export const MONTHS_ES = [
  "Enero", "Febrero", "Marzo", "Abril", "Mayo", "Junio",
  "Julio", "Agosto", "Septiembre", "Octubre", "Noviembre", "Diciembre",
];
export const DAYS_ES = ["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"];
