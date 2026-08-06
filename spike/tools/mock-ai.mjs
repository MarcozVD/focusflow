// Mock OpenAI-compatible para E2E de FocusFlow (puerto 9410)
import http from "node:http";

const PORT = 9410;

function respond(req, messages, res) {
  const user = (messages.find((m) => m.role === "user") || {}).content || "";
  const lower = user.toLowerCase();

  let answer;
  if (lower.includes("reunión") || lower.includes("reunion")) {
    answer = {
      is_relevant: true,
      confidence: 0.95,
      reason: "Reunión con fecha y hora concretas",
      events: [
        {
          title: "Reunión de equipo",
          description: "Reunión semanal del equipo",
          category: "Trabajo",
          start_date: "2026-08-07",
          end_date: "2026-08-07",
          start_time: "15:00",
          end_time: "16:00",
          priority: "Alta",
          location: "Sala 3",
          tags: [],
        },
      ],
    };
  } else if (lower.includes("informe")) {
    answer = {
      is_relevant: true,
      confidence: 0.8,
      reason: "Entrega de informe con plazo",
      events: [
        {
          title: "Entregar informe mensual",
          description: "Plazo límite de entrega del informe",
          category: "Trabajo",
          start_date: "2026-08-10",
          end_date: "2026-08-10",
          start_time: "18:00",
          end_time: "19:00",
          priority: "Media",
          location: "",
          tags: [],
        },
      ],
    };
  } else if (lower.includes("seguimiento")) {
    answer = {
      is_relevant: true,
      confidence: 0.9,
      reason: "Reunión de seguimiento con fecha y hora",
      events: [
        {
          title: "Seguimiento del proyecto",
          description: "Seguimiento con la jefa",
          category: "Trabajo",
          start_date: "2026-08-13",
          end_date: "2026-08-13",
          start_time: "10:00",
          end_time: "11:00",
          priority: "Alta",
          location: "Sala 2",
          tags: [],
        },
      ],
    };
  } else if (lower.includes("revisión") || lower.includes("revision")) {
    answer = {
      is_relevant: true,
      confidence: 0.85,
      reason: "Revisión trimestral programada",
      events: [
        {
          title: "Revisión trimestral",
          description: "Preparar documentación",
          category: "Trabajo",
          start_date: "2026-08-14",
          end_date: "2026-08-14",
          start_time: "09:00",
          end_time: "10:00",
          priority: "Media",
          location: "",
          tags: [],
        },
      ],
    };
  } else if (lower.includes("boletín") || lower.includes("boletin")) {
    answer = { is_relevant: false, confidence: 0.99, reason: "Boletín informativo sin eventos", events: [] };
  } else {
    answer = { is_relevant: false, confidence: 0.5, reason: "Sin información de calendario", events: [] };
  }

  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(
    JSON.stringify({
      id: "mock-1",
      object: "chat.completion",
      model: "mock-model",
      choices: [{ index: 0, message: { role: "assistant", content: JSON.stringify(answer) }, finish_reason: "stop" }],
      usage: { prompt_tokens: 10, completion_tokens: 10, total_tokens: 20 },
    }),
  );
  console.log(`[mock-ai] ${user.slice(0, 90).replace(/\n/g, " ")} -> ${answer.is_relevant ? "relevante" : "no relevante"}`);
}

http
  .createServer((req, res) => {
    if (req.method !== "POST" || !req.url.endsWith("/chat/completions")) {
      res.writeHead(404);
      return res.end();
    }
    let body = "";
    req.on("data", (c) => (body += c));
    req.on("end", () => {
      try {
        const { messages } = JSON.parse(body);
        respond(req, messages, res);
      } catch (e) {
        res.writeHead(500);
        res.end(JSON.stringify({ error: String(e) }));
      }
    });
  })
  .listen(PORT, "127.0.0.1", () => console.log(`[mock-ai] OpenAI-compatible en http://127.0.0.1:${PORT}`));
