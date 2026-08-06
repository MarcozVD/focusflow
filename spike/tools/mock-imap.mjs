// Mock IMAP para E2E de FocusFlow (puerto 1143)
import net from "node:net";

const PORT = 1143;

const now = new Date();
const dateStr = now.toUTCString();

function buildMail(uid, from, subject, body) {
  return (
    `From: ${from}\r\n` +
    `To: usuario@test.local\r\n` +
    `Subject: ${subject}\r\n` +
    `Date: ${dateStr}\r\n` +
    `Message-ID: <mock-${uid}@test.local>\r\n` +
    `MIME-Version: 1.0\r\n` +
    `Content-Type: text/plain; charset=utf-8\r\n` +
    `\r\n` +
    body
  );
}

const MAILS = [
  {
    uid: 1,
    body: buildMail(1, "Jefa del Equipo <jefa@empresa.test>",
      "Reunión de equipo el viernes",
      "Hola,\n\nRecordatorio: la reunión de equipo será el viernes 7 de agosto a las 15:00 en la sala 3. Duración una hora.\n\nSaludos, Laura.",
    ),
  },
  {
    uid: 2,
    body: buildMail(2, "Gerencia <gerencia@empresa.test>",
      "Informe mensual: entrega",
      "Te recordamos que la entrega del informe mensual es el lunes 10 de agosto antes de las 18:00.\n\nGerencia.",
    ),
  },
  {
    uid: 3,
    body: buildMail(3, "Noticias <noreply@boletin.test>",
      "Boletín semanal",
      "Estas son las novedades de la semana: nuevo software, ofertas, eventos de la comunidad.\n\nNo responda a este correo.",
    ),
  },
];

// Si existe el flag, el mock añade correos nuevos (fases 2 y 3 del E2E).
import fs from "node:fs";
const FLAG = "mock-extra.flag";
const EXTRAS = [
  {
    uid: 4,
    body: buildMail(4, "Jefa del Equipo <jefa@empresa.test>",
      "Seguimiento del proyecto",
      "Hola,\n\nNos vemos el jueves 13 de agosto a las 10:00 en la sala 2 para el seguimiento del proyecto.\n\nLaura.",
    ),
  },
  {
    uid: 5,
    body: buildMail(5, "Gerencia <gerencia@empresa.test>",
      "Revisión trimestral",
      "La revisión trimestral será el viernes 14 de agosto a las 09:00. Preparad la documentación.\n\nGerencia.",
    ),
  },
];
if (fs.existsSync(FLAG)) MAILS.push(...EXTRAS);

let tagCounter = 0;

function respond(sock, line) {
  sock.write(line + "\r\n");
}

const server = net.createServer((sock) => {
  sock.write(`* OK mock imap ready\r\n`);
  let buffer = "";
  const state = { selected: false, uidvalidity: 9999 };

  sock.on("data", (chunk) => {
    buffer += chunk.toString("utf8");
    let idx;
    while ((idx = buffer.indexOf("\r\n")) >= 0) {
      const line = buffer.slice(0, idx);
      buffer = buffer.slice(idx + 2);
      if (line.trim() === "") continue;
      const tag = line.split(" ")[0];
      const rest = line.slice(tag.length + 1).trim();
      const cmd = rest.toUpperCase();

      if (cmd.startsWith("LOGIN")) {
        respond(sock, `${tag} OK LOGIN completed`);
      } else if (cmd.startsWith("CAPABILITY")) {
        respond(sock, `* CAPABILITY IMAP4rev1 AUTH=PLAIN`);
        respond(sock, `${tag} OK CAPABILITY completed`);
      } else if (cmd.startsWith("SELECT")) {
        state.selected = true;
        respond(sock, `* ${MAILS.length} EXISTS`);
        respond(sock, `* OK [UIDVALIDITY ${state.uidvalidity}] UIDs valid`);
        respond(sock, `* OK [UIDNEXT ${MAILS.length + 1}] Next UID`);
        respond(sock, `${tag} OK [READ-WRITE] SELECT completed`);
      } else if (cmd.startsWith("UID SEARCH")) {
        const expr = rest.slice("UID SEARCH ".length).trim();
        let ids = MAILS.map((m) => m.uid);
        const m = expr.match(/^UID (\d+):\*$/);
        if (m) {
          const from = parseInt(m[1], 10);
          ids = ids.filter((u) => u >= from);
        }
        respond(sock, `* SEARCH ${ids.join(" ")}`);
        respond(sock, `${tag} OK SEARCH completed`);
      } else if (cmd.startsWith("UID FETCH")) {
        const match = rest.match(/^UID FETCH (.+?) \((.+)\)$/i);
        if (match) {
          const uids = match[1].split(",").map((u) => parseInt(u.trim(), 10));
          for (const uid of uids) {
            const mail = MAILS.find((x) => x.uid === uid);
            if (!mail) continue;
            const body = mail.body;
            respond(sock, `* ${uid} FETCH (UID ${uid} BODY[] {${Buffer.byteLength(body, "utf8")}}`);
            sock.write(body + ")\r\n");
          }
          respond(sock, `${tag} OK FETCH completed`);
        } else {
          respond(sock, `${tag} BAD cannot parse FETCH`);
        }
      } else if (cmd.startsWith("NOOP")) {
        respond(sock, `* OK`);
        respond(sock, `${tag} OK NOOP completed`);
      } else if (cmd.startsWith("LOGOUT")) {
        respond(sock, `* BYE mock logout`);
        respond(sock, `${tag} OK LOGOUT completed`);
        sock.end();
      } else {
        respond(sock, `${tag} BAD unknown command: ${cmd}`);
      }
    }
  });
});

server.listen(PORT, "127.0.0.1", () => console.log(`[mock-imap] IMAP en 127.0.0.1:${PORT}`));
