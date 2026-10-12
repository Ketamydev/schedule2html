use serde::Serialize;
use std::{
    env,
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

type Res<T> = Result<T, Box<dyn Error>>;

#[derive(Serialize)]
struct Lesson {
    subject: String,
    room: String,
    teacher: String,
}

#[derive(Serialize)]
struct Row {
    n: u32,
    cells: Vec<Option<Lesson>>,
}

#[derive(Serialize)]
struct Section {
    groups: Vec<String>,
    rows: Vec<Row>,
}

#[derive(Serialize)]
struct Day {
    date: String,
    sections: Vec<Section>,
}

const TEMPLATE: &str = include_str!("template.html");
const DEFAULT_CONFIG: &str = include_str!("config.js");
const PLACES: [&str; 4] = ["Библиотека", "Спортзал", "Актовый зал", "Столовая"];
const MAX_DOCX_BYTES: u64 = 100 * 1024 * 1024;
const MAX_XML_BYTES: usize = 50 * 1024 * 1024;
const MAX_TABLES: usize = 64;
const MAX_ROWS: usize = 64;
const MAX_GROUPS: usize = 64;
const MAX_SPAN: usize = 64;
const MAX_PAIR: u32 = 32;
const MAX_TEXT: usize = 2000;

fn main() {
    let interactive = env::args().nth(1).is_none();
    if let Err(e) = run() {
        eprintln!("\nОшибка: {e}");
        eprintln!("Подсказка: schedule2html --help");
        pause(interactive);
        std::process::exit(1);
    }
    pause(interactive);
}

fn pause(yes: bool) {
    if !yes {
        return;
    }
    eprintln!("\nГотово. Нажмите Enter, чтобы закрыть...");
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
}

struct Opts {
    out: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    open: bool,
    recursive: bool,
    quiet: bool,
    inputs: Vec<PathBuf>,
}

fn run() -> Res<()> {
    let opts = parse_args()?;

    let dir = env::current_dir()?;
    if !opts.quiet {
        eprintln!("Папка: {}", dir.display());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for input in &opts.inputs {
        collect_docx(input, opts.recursive, &mut files)?;
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err("не нашёл ни одного .docx в этой папке".into());
    }
    if !opts.quiet {
        eprintln!("Найдено файлов: {}\n", files.len());
    }
    if files.len() > 1 && opts.out.is_some() {
        return Err("-o работает только с одним входным файлом".into());
    }

    let mut written: Vec<PathBuf> = Vec::new();
    let mut failed = 0usize;
    let total = files.len();
    for (i, input) in files.iter().enumerate() {
        let day = match parse_docx(input) {
            Ok(d) if !d.sections.is_empty() => d,
            Ok(_) => {
                eprintln!(
                    "! {}: таблицы с расписанием не найдены — пропускаю",
                    input.display()
                );
                failed += 1;
                continue;
            }
            Err(e) => {
                eprintln!("! {}: {e} — пропускаю", input.display());
                failed += 1;
                continue;
            }
        };
        let target = output_path(input, &opts);
        if let Some(dir) = target.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }
        let json = serde_json::to_string(&day)?.replace("</", "<\\/");
        let html = TEMPLATE
            .replace("__DATA__", &json)
            .replace("__CONFIG__", &load_config()?);
        fs::write(&target, html)?;
        if !opts.quiet {
            eprintln!(
                "[{}/{total}] {} → {}",
                i + 1,
                input.display(),
                target.display()
            );
        }
        written.push(target);
    }
    if written.is_empty() {
        return Err("ничего не удалось обработать".into());
    }
    if !opts.quiet {
        eprintln!(
            "\nГотово: {}, пропущено: {} (из {total})",
            written.len(),
            failed
        );
    }
    if opts.open {
        open_in_browser(&written[0])?;
    }
    Ok(())
}

fn parse_args() -> Res<Opts> {
    let mut o = Opts {
        out: None,
        out_dir: None,
        open: false,
        recursive: false,
        quiet: false,
        inputs: Vec::new(),
    };
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" | "--output" => {
                o.out = Some(PathBuf::from(args.next().ok_or("после -o нужен путь")?))
            }
            "--out-dir" => {
                o.out_dir = Some(PathBuf::from(
                    args.next().ok_or("после --out-dir нужна папка")?,
                ))
            }
            "--open" => o.open = true,
            "-r" | "--recursive" => o.recursive = true,
            "-q" | "--quiet" => o.quiet = true,
            "-h" | "--help" => {
                println!("{HELP}");
                std::process::exit(0);
            }
            _ if a.starts_with('-') => {
                return Err(format!("неизвестный параметр «{a}». schedule2html --help").into())
            }
            _ => o.inputs.push(PathBuf::from(a)),
        }
    }
    if o.inputs.is_empty() {
        o.inputs.push(PathBuf::from("."));
    }
    Ok(o)
}

const HELP: &str = "\
schedule2html — Word-расписания (.docx) в одну HTML-страницу

Использование:
  schedule2html [параметры] [<файл.docx | папка> ...]

Без аргументов берёт все .docx из текущей папки и кладёт .html рядом.

Параметры:
  -o, --output <файл.html>   куда писать (только для одного входа)
      --out-dir <папка>      папка для html (по умолчанию — рядом с .docx)
      --open                 открыть результат в браузере
  -r, --recursive            искать .docx в подпапках
  -q, --quiet                без прогресса
  -h, --help                 справка

Примеры:
  schedule2html
  schedule2html 07.10.2026.docx
  schedule2html --open 07.10.2026.docx
  schedule2html -r C:\\Расписание
  schedule2html 02.09.2026.docx -o pagina.html";

fn collect_docx(path: &Path, recursive: bool, out: &mut Vec<PathBuf>) -> Res<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let p = entry?.path();
            if p.is_dir() {
                if recursive {
                    collect_docx(&p, recursive, out)?;
                }
            } else if is_docx(&p) {
                out.push(p);
            }
        }
    } else if path.exists() {
        out.push(path.to_path_buf());
    } else {
        return Err(format!("{}: файл или папка не найдены", path.display()).into());
    }
    Ok(())
}

fn is_docx(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("docx"))
}

fn load_config() -> Res<String> {
    let external = exe_path()?.join("config.js");
    if external.exists() {
        return Ok(fs::read_to_string(external)?);
    }
    Ok(DEFAULT_CONFIG.to_string())
}

fn exe_path() -> Res<PathBuf> {
    let mut p = env::current_exe()?;
    p.pop();
    Ok(p)
}

fn output_path(input: &Path, o: &Opts) -> PathBuf {
    if let Some(out) = &o.out {
        return out.clone();
    }
    let name = input.file_stem().unwrap_or_default();
    match &o.out_dir {
        Some(dir) => dir.join(name).with_extension("html"),
        None => input.with_extension("html"),
    }
}

fn open_in_browser(path: &Path) -> Res<()> {
    let path = fs::canonicalize(path)?;
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", ""])
        .arg(&path)
        .spawn()?;
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&path).spawn()?;
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(&path).spawn()?;
    Ok(())
}

fn parse_docx(path: &Path) -> Res<Day> {
    let size = fs::metadata(path)?.len();
    if size > MAX_DOCX_BYTES {
        return Err(format!("файл слишком большой ({size} байт), лимит {MAX_DOCX_BYTES}").into());
    }
    let mut zip = zip::ZipArchive::new(fs::File::open(path)?)?;
    let mut entry = zip.by_name("word/document.xml")?;
    if entry.size() > MAX_XML_BYTES as u64 {
        return Err("document.xml слишком большой — пропускаю".into());
    }
    let mut xml = Vec::new();
    entry
        .by_ref()
        .take(MAX_XML_BYTES as u64 + 1)
        .read_to_end(&mut xml)?;
    if xml.len() > MAX_XML_BYTES {
        return Err("document.xml слишком большой — пропускаю".into());
    }
    let xml = String::from_utf8(xml).map_err(|_| "document.xml не UTF-8")?;
    let doc = roxmltree::Document::parse(&xml)?;
    let name = |n: roxmltree::Node, s: &str| n.is_element() && n.tag_name().name() == s;

    let title = doc
        .descendants()
        .filter(|n| name(*n, "p") && !n.ancestors().skip(1).any(|a| name(a, "tbl")))
        .map(text_of)
        .find(|t| t.contains("Расписание"));
    let date = title
        .as_deref()
        .and_then(|t| date_from(t.split("на").last().unwrap_or(t)))
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .and_then(date_from)
        })
        .ok_or("не нашёл дату ни в заголовке, ни в имени файла")?;

    let mut sections: Vec<Section> = Vec::new();
    let tables: Vec<roxmltree::Node> = doc
        .descendants()
        .filter(|n| name(*n, "tbl") && !n.ancestors().skip(1).any(|a| name(a, "tbl")))
        .take(MAX_TABLES + 1)
        .collect();
    if tables.len() > MAX_TABLES {
        return Err(format!("слишком много таблиц (>{MAX_TABLES}) — пропускаю").into());
    }
    for tbl in tables {
        let mut cur: Option<Section> = None;
        let keep: Vec<bool> = tbl
            .children()
            .find(|n| name(*n, "tblGrid"))
            .map(|g| {
                g.children()
                    .filter(|n| name(*n, "gridCol"))
                    .map(|c| {
                        c.attributes()
                            .find(|a| a.name() == "w")
                            .and_then(|a| a.value().parse::<i32>().ok())
                            .is_none_or(|w| w >= 200)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut vcarry: Vec<Option<String>> = Vec::new();
        let mut rows_seen = 0usize;
        for tr in tbl.children().filter(|n| name(*n, "tr")) {
            rows_seen += 1;
            if rows_seen > MAX_ROWS {
                return Err(format!("слишком много строк в таблице (>{MAX_ROWS}) — пропускаю").into());
            }
            let (mut cells, merges) = row_cells(tr, &keep);
            if cells.is_empty() {
                continue;
            }
            if vcarry.len() < cells.len() {
                vcarry.resize(cells.len(), None);
            }
            for i in 0..cells.len() {
                match merges.get(i).copied().flatten() {
                    Some(true) => {
                        let t = truncate(&cells[i]);
                        vcarry[i] = if t.is_empty() { None } else { Some(t) };
                    }
                    Some(false) => {
                        if norm(&cells[i]).is_empty() {
                            if let Some(t) = vcarry[i].clone() {
                                cells[i] = t;
                            }
                        }
                    }
                    None => {
                        vcarry[i] = None;
                    }
                }
            }
            let first = norm(&cells[0]);
            if let Ok(n) = first.parse::<u32>() {
                if n > MAX_PAIR {
                    return Err(format!("номер пары {n} больше лимита {MAX_PAIR} — пропускаю").into());
                }
                if let Some(s) = cur.as_mut() {
                    if s.groups.len() > MAX_GROUPS {
                        return Err(format!("слишком много групп (>{MAX_GROUPS}) — пропускаю").into());
                    }
                    let lessons = (0..s.groups.len())
                        .map(|i| cells.get(i + 1).and_then(|c| parse_lesson(c)))
                        .collect();
                    s.rows.push(Row { n, cells: lessons });
                }
            } else if cells.iter().skip(1).any(|c| !norm(c).is_empty()) {
                if let Some(s) = cur.take() {
                    sections.push(s);
                }
                let groups = cells[1..]
                    .iter()
                    .map(|c| truncate(&norm(c).replace(' ', "")))
                    .filter(|g| !g.is_empty())
                    .collect();
                cur = Some(Section {
                    groups,
                    rows: vec![],
                });
            }
        }
        if let Some(s) = cur {
            sections.push(s);
        }
    }
    sections.retain(|s| !s.groups.is_empty());
    Ok(Day { date, sections })
}

fn row_cells(tr: roxmltree::Node, keep: &[bool]) -> (Vec<String>, Vec<Option<bool>>) {
    let mut v: Vec<String> = Vec::new();
    let mut m: Vec<Option<bool>> = Vec::new();
    for tc in tr
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "tc")
    {
        let span = tc
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "gridSpan")
            .and_then(|n| n.attributes().find(|a| a.name() == "val"))
            .and_then(|a| a.value().parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, MAX_SPAN);
        let merge = tc
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "tcPr")
            .and_then(|p| {
                p.children()
                    .find(|n| n.is_element() && n.tag_name().name() == "vMerge")
            })
            .map(|v| v.attribute("val") == Some("restart"));
        let t = text_of(tc);
        for _ in 0..span {
            v.push(t.clone());
            m.push(merge);
        }
    }
    if v.len() == keep.len() {
        let zipped: Vec<(String, Option<bool>)> = v
            .into_iter()
            .zip(m)
            .zip(keep)
            .filter(|((_, _), k)| **k)
            .map(|((t, mg), _)| (t, mg))
            .collect();
        v = zipped.iter().map(|(t, _)| t.clone()).collect();
        m = zipped.iter().map(|(_, mg)| *mg).collect();
    }
    (v, m)
}

fn truncate(s: &str) -> String {
    if s.len() <= MAX_TEXT {
        return s.to_string();
    }
    let mut end = MAX_TEXT;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn text_of(node: roxmltree::Node) -> String {
    let mut s = String::new();
    for n in node.descendants().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "t" => s.push_str(n.text().unwrap_or("")),
            "tab" | "br" | "p" => s.push(' '),
            _ => {}
        }
    }
    s
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn date_from(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .map(|c| if c == '_' || c == '-' { '.' } else { c })
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let parts: Vec<&str> = cleaned.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return None;
    }
    let (d, m, y): (u32, u32, u32) = (
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    );
    if !(1..=31).contains(&d) || !(1..=12).contains(&m) || y < 1000 {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

fn parse_lesson(raw: &str) -> Option<Lesson> {
    let s = norm(raw);
    if s.is_empty() {
        return None;
    }
    let (before, teacher) = match s.rfind('(') {
        Some(p) => {
            let t = s[p + 1..].split(')').next().unwrap_or("");
            (
                s[..p].trim().to_string(),
                t.trim().trim_matches(',').trim().to_string(),
            )
        }
        None => (s.clone(), String::new()),
    };

    let mut cut = before.find("каб");
    for p in PLACES {
        if let Some(i) = before.find(p) {
            cut = Some(cut.map_or(i, |c| c.min(i)));
        }
    }
    let (subject, room_raw) = match cut {
        Some(i) => (before[..i].trim(), before[i..].trim()),
        None => (before.trim(), ""),
    };
    let room = match room_raw.strip_prefix("каб") {
        Some(rest) => {
            let r = rest.trim_matches(|c: char| c == '.' || c.is_whitespace());
            if r.is_empty() {
                "каб.".to_string()
            } else {
                format!("каб. {r}")
            }
        }
        None => room_raw
            .trim_matches(|c: char| c == '.' || c.is_whitespace())
            .to_string(),
    };
    Some(Lesson {
        subject: subject.to_string(),
        room,
        teacher,
    })
}
