//! Benchmarks de rendimiento (fase 13). NO son tests: miden y reportan.
//! Ejecutar: `cargo test --release --test perf_bench -- --ignored --nocapture`
//!
//! Miden: operaciones de DB (insert/query/overlap/dedupe/export), costo del
//! motor de calendario (conflictos/tiempo libre) y el coste de los handlers
//! de IPC (la capa que envuelve cada comando; el transporte WebView2 local
//! añade ~0.1–1 ms por invocación).

use focusflow_spike_lib::planning::engine_with_calendar;
use focusflow_spike_lib::store::Db;
use std::time::Instant;

const N_TASKS: usize = 2000;
const DAY: i64 = 86_400_000;
const HOUR: i64 = 3_600_000;

fn bench_db() -> Db {
    let db = Db::open_memory_pub().unwrap();
    db.wipe_data().unwrap(); // sin tareas demo: solo las del benchmark
    db
}

fn fill(db: &Db, n: usize) {
    let base = chrono::Local::now().timestamp_millis();
    for i in 0..n {
        let start = base + (i as i64) % 90 * DAY + (i % 20) as i64 * HOUR;
        let end = start + 3_600_000 + (i % 3) as i64 * HOUR;
        db.create(&format!("Tarea {i}"), "uni", "media", start, end, false)
            .unwrap();
    }
}

fn ms(name: &str, iters: usize, f: impl Fn()) {
    let t0 = Instant::now();
    for _ in 0..iters {
        f();
    }
    let per = t0.elapsed().as_micros() as f64 / iters as f64;
    println!("{name:<42} {per:>10.1} µs/op   ({iters} ops en {:.1} ms)", t0.elapsed().as_millis());
}

#[test]
#[ignore]
fn perf_report() {
    let db = bench_db();
    println!("\n===== FocusFlow perf report =====");

    let t0 = Instant::now();
    fill(&db, N_TASKS);
    println!("{:<42} {:>10.1} µs/op   (insertar {N_TASKS} tareas)", "db.create (2000)", t0.elapsed().as_micros() as f64 / N_TASKS as f64);

    ms("db.list (2000 filas)", 100, || {
        assert_eq!(db.list().unwrap().len(), N_TASKS);
    });
    ms("db.list_range (ventana 1 día)", 200, || {
        db.list_range(chrono::Local::now().timestamp_millis(), chrono::Local::now().timestamp_millis() + 24 * HOUR)
            .unwrap();
    });
    ms("db.find_overlap (conflictos)", 1000, || {
        db.find_overlap(-1, chrono::Local::now().timestamp_millis() + 45 * DAY, chrono::Local::now().timestamp_millis() + 45 * DAY + HOUR)
            .unwrap();
    });
    ms("db.find_similar_suggestion (dedupe)", 500, || {
        let _ = db.find_similar_suggestion("Tarea 999", None, None, Some("x"));
    });
    ms("db.insert_suggestion", 200, || {
        db.insert_suggestion(
            "email", Some("m"), Some("s"), "asunto", "task", "Título", "", "uni",
            "media", None, None, None, 0, "", "[]", 0.8, "r", None, "", "pending",
        )
        .unwrap();
    });
    ms("db.export_data (JSON completo)", 20, || {
        db.export_data().unwrap();
    });

    // motor de calendario (base del render): tiempo libre con 2000 tareas
    let engine = engine_with_calendar(&db);
    let today_start = {
        let d = chrono::Local::now().date_naive();
        focusflow_spike_lib::engine::local_ms(d.and_hms_opt(0, 0, 0).unwrap())
    };
    ms("engine.available_minutes (día, 2000 tareas)", 200, || {
        engine.available_minutes(today_start, today_start + 24 * HOUR);
    });

    // coste del handler de IPC (sin transporte; el transporte local WebView2
    // suma ~0.1–1 ms). `task_list` y `sync_status` comparten esta forma.
    ms("handler-equivalente task_list (serialización)", 500, || {
        let rows = db.list().unwrap();
        let _ = serde_json::to_string(&rows).unwrap().len();
    });

    // vaciado del benchmark
    db.wipe_data().unwrap();
    println!("=================================");
}
