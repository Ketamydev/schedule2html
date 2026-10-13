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
    full: bool,
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
struct ParseWarning {
    section: String,
    pair: u32,
    group: String,
    msg: String,
    raw: String,
}
const TEMPLATE: &str = include_str!("template.tsx");
const PREVIEW: &str = include_str!("preview.html");
const DEFAULT_CONFIG: &str = include_str!("config.js");
const PLACES: [&str; 9] = [
    "библиотека",
    "спортзал",
    "спортивный зал",
    "актовый зал",
    "столовая",
    "медкабинет",
    "музей",
    "бассейн",
    "тир",
];
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
    let config = load_config(!opts.quiet)?;
    for (i, input) in files.iter().enumerate() {
        let (day, warns) = match parse_docx(input) {
            Ok((d, w)) if !d.sections.is_empty() => (d, w),
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
        if !opts.quiet {
            for w in &warns {
                eprintln!(
                    "  ~ {}: секция «{}», пара {}, группа «{}»: {}; ячейка «{}»",
                    input.display(),
                    w.section,
                    w.pair,
                    w.group,
                    w.msg,
                    w.raw,
                );
            }
        }
        let target = output_path(input, &opts);
        if let Some(dir) = target.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)?;
            }
        }
        let json = serde_json::to_string(&day)?.replace("</", "<\\/");
        let usercfg = usercfg_object(&config)?;
        let gen_html = config_flag(&config, "genHtml", true);
        let tsx = TEMPLATE
            .replace("__DATA__", &json)
            .replace("__USERCFG__", &usercfg);
        fs::write(&target, tsx)?;
        let opened = if gen_html {
            let preview_target = preview_path(&target, &opts);
            let preview_html = PREVIEW
                .replace("__DATA__", &json)
                .replace("__CONFIG__", &config);
            fs::write(&preview_target, preview_html)?;
            if !opts.quiet {
                eprintln!(
                    "[{}/{total}] {} → {}, {}",
                    i + 1,
                    input.display(),
                    target.display(),
                    preview_target.display()
                );
            }
            preview_target
        } else {
            if !opts.quiet {
                eprintln!(
                    "[{}/{total}] {} → {}",
                    i + 1,
                    input.display(),
                    target.display()
                );
            }
            target
        };
        written.push(opened);
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
schedule2html — Word-расписания (.docx) в React/TSX-страницу + HTML для просмотра
Использование:
  schedule2html [параметры] [<файл.docx | папка> ...]
Без аргументов берёт все .docx из текущей папки и кладёт .tsx и .html рядом.
Генерация .html включается флагом genHtml в config.js (по умолчанию true).
Параметры:
  -o, --output <файл.tsx>    куда писать tsx (html рядом, только для одного входа)
      --out-dir <папка>      папка для tsx и html (по умолчанию — рядом с .docx)
      --open                 открыть результат (html, если genHtml, иначе tsx)
  -r, --recursive            искать .docx в подпапках
  -q, --quiet                без прогресса
  -h, --help                 справка
Примеры:
  schedule2html
  schedule2html 07.10.2026.docx
  schedule2html --open 07.10.2026.docx
  schedule2html -r C:\\Расписание
  schedule2html 02.09.2026.docx -o pagina.tsx";
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
fn load_config(verbose: bool) -> Res<String> {
    let external = exe_path()?.join("config.js");
    if external.exists() {
        let raw = fs::read(&external)?;
        let text = String::from_utf8(raw)
            .map_err(|_| format!("{}: файл должен быть в кодировке UTF-8", external.display()))?;
        if !text.contains("USERCFG") {
            return Err(format!("{}: не найден объект USERCFG", external.display()).into());
        }
        return Ok(text);
    }
    fs::write(&external, DEFAULT_CONFIG)?;
    if verbose {
        eprintln!(
            "Создан {} — настройки страницы меняются в нём",
            external.display()
        );
    }
    Ok(DEFAULT_CONFIG.to_string())
}
fn usercfg_object(config_js: &str) -> Res<String> {
    let anchor = config_js
        .find("USERCFG")
        .ok_or("в config.js не найден объект USERCFG")?;
    let after = &config_js[anchor..];
    let start_rel = after.find('{').ok_or("в config.js нет тела USERCFG")?;
    let start = anchor + start_rel;
    let end = config_js.rfind('}').ok_or("в config.js нет конца USERCFG")?;
    if end <= start {
        return Err("в config.js повреждён объект USERCFG".into());
    }
    Ok(config_js[start..=end].to_string())
}
fn config_flag(config_js: &str, key: &str, default: bool) -> bool {
    let mut from = 0;
    while let Some(rel) = config_js[from..].find(key) {
        let i = from + rel;
        let left_ok = config_js[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$'));
        if left_ok {
            let after_key = &config_js[i + key.len()..];
            let after_key = after_key.trim_start();
            if let Some(rest) = after_key.strip_prefix(':') {
                let rest = rest.trim_start();
                if rest.starts_with("true") {
                    return true;
                }
                if rest.starts_with("false") {
                    return false;
                }
            }
        }
        from = i + 1;
        if from >= config_js.len() {
            break;
        }
    }
    default
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
        Some(dir) => dir.join(name).with_extension("tsx"),
        None => input.with_extension("tsx"),
    }
}
fn preview_path(tsx_target: &Path, o: &Opts) -> PathBuf {
    if let Some(out) = &o.out {
        let s = out.to_string_lossy();
        if s.ends_with(".tsx") {
            return PathBuf::from(format!("{}.html", &s[..s.len() - 4]));
        }
        if s.ends_with(".html") {
            return out.clone();
        }
        return PathBuf::from(format!("{s}.html"));
    }
    tsx_target.with_extension("html")
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
fn parse_docx(path: &Path) -> Res<(Day, Vec<ParseWarning>)> {
    let mut warns: Vec<ParseWarning> = Vec::new();
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
                return Err(
                    format!("слишком много строк в таблице (>{MAX_ROWS}) — пропускаю").into(),
                );
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
                    return Err(
                        format!("номер пары {n} больше лимита {MAX_PAIR} — пропускаю").into(),
                    );
                }
                if let Some(s) = cur.as_mut() {
                    if s.groups.len() > MAX_GROUPS {
                        return Err(
                            format!("слишком много групп (>{MAX_GROUPS}) — пропускаю").into()
                        );
                    }
                    let mut lessons: Vec<Option<Lesson>> = Vec::new();
                    for i in 0..s.groups.len() {
                        let raw = cells.get(i + 1).map(|c| c.as_str()).unwrap_or("");
                        let (lesson, warn) = parse_lesson(raw);
                        let grp = s.groups.get(i).cloned().unwrap_or("?".to_string());
                        if let Some(w) = warn {
                            warns.push(ParseWarning {
                                section: s.groups.join("/"),
                                pair: n,
                                group: grp.clone(),
                                msg: w,
                                raw: truncate(&norm(raw)),
                            });
                        }
                        if lesson.is_none() && !norm(raw).is_empty() {
                            warns.push(ParseWarning {
                                section: s.groups.join("/"),
                                pair: n,
                                group: grp,
                                msg: "не разобрана, показана как текст".to_string(),
                                raw: truncate(&norm(raw)),
                            });
                            lessons.push(Some(Lesson {
                                subject: truncate(&norm(raw)),
                                room: String::new(),
                                teacher: String::new(),
                                full: false,
                            }));
                        } else {
                            lessons.push(lesson);
                        }
                    }
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
    Ok((Day { date, sections }, warns))
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
fn parse_lesson(raw: &str) -> (Option<Lesson>, Option<String>) {
    let s = norm(raw);
    if s.is_empty() {
        return (None, None);
    }
    let mut warnings: Vec<String> = Vec::new();
    let mut rest = s.clone();
    let mut teacher = String::new();
    if let Some(p) = rest.rfind('(') {
        match rest[p..].find(')') {
            Some(q_rel) => {
                let q = p + q_rel;
                let inside = rest[p + 1..q].trim().trim_matches(',').trim();
                if inside.is_empty() {
                    warnings.push("пустые скобки () — преподаватель не найден".to_string());
                } else {
                    teacher = inside.to_string();
                    if teacher.chars().any(|c| c.is_ascii_digit()) {
                        warnings.push(format!(
                            "в имени преподавателя есть цифры: «{teacher}»"
                        ));
                    }
                }
                let before = rest[..p].trim();
                let after = rest[q + 1..].trim();
                rest = match (before.is_empty(), after.is_empty()) {
                    (true, true) => String::new(),
                    (true, false) => after.to_string(),
                    (false, true) => before.to_string(),
                    (false, false) => format!("{before} {after}"),
                };
            }
            None => {
                warnings.push(
                    "незакрытая скобка ( — текст после неё принят за преподавателя"
                        .to_string(),
                );
                teacher = rest[p + 1..].trim().trim_matches(',').trim().to_string();
                rest = rest[..p].trim().to_string();
            }
        }
    }
    let (subj_part, room, room_warn) = extract_room(&rest);
    if let Some(w) = room_warn {
        warnings.push(w);
    }
    let mut subject = subj_part;
    if teacher.is_empty() {
        if let Some((s2, t2, guessed, warn_only)) = split_trailing_teacher(&subject) {
            if warn_only {
                warnings.push(format!(
                    "конец похож на фамилию, но деление ненадёжно — проверьте ячейку: «{s2}»"
                ));
            } else {
                subject = s2;
                teacher = t2;
                if guessed {
                    warnings.push(format!(
                        "преподаватель без скобок угадан как «{teacher}» — проверьте ячейку"
                    ));
                }
            }
        }
    }
    let subject = subject.trim().to_string();
    let room = room.trim().to_string();
    teacher = teacher.trim().trim_matches(',').trim().to_string();
    if subject.is_empty() && room.is_empty() && teacher.is_empty() {
        let warn = if warnings.is_empty() {
            None
        } else {
            Some(warnings.join("; "))
        };
        return (None, warn);
    }
    if subject.is_empty() && room.is_empty() {
        warnings.push("пустой предмет — остался только преподаватель".to_string());
    }
    let full = subject.is_empty() && !room.is_empty();
    let warn = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("; "))
    };
    (
        Some(Lesson {
            subject,
            room,
            teacher,
            full,
        }),
        warn,
    )
}
fn extract_room(s: &str) -> (String, String, Option<String>) {
    if s.trim().is_empty() {
        return (s.to_string(), String::new(), None);
    }
    let orig: Vec<char> = s.chars().collect();
    let lower: Vec<char> = s.to_lowercase().chars().collect();
    fn find_sub(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
        if needle.is_empty() || from + needle.len() > hay.len() {
            return None;
        }
        (from..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
    }
    let substr = |v: &[char], a: usize, b: usize| -> String { v[a..b].iter().collect() };
    enum Kind {
        Marker(&'static str),
        Place(usize),
    }
    let mut best: Option<(usize, usize, Kind)> = None;
    let mut consider = |idx: usize, len: usize, k: Kind| {
        let better = match &best {
            None => true,
            Some((bi, bl, _)) => idx < *bi || (idx == *bi && len > *bl),
        };
        if better {
            best = Some((idx, len, k));
        }
    };
    const MARKERS: [(&str, &str); 6] = [
        ("аудитория", "ауд."),
        ("лаборатория", "лаб."),
        ("кабинет", "каб."),
        ("ауд", "ауд."),
        ("лаб", "лаб."),
        ("каб", "каб."),
    ];
    for (m, norm_m) in MARKERS {
        let mch: Vec<char> = m.chars().collect();
        let mut from = 0;
        while let Some(i) = find_sub(&lower, &mch, from) {
            let left_ok = i == 0 || !orig[i - 1].is_alphanumeric();
            let right_ok = match lower.get(i + mch.len()) {
                None => true,
                Some(&c) => !c.is_alphanumeric() || c == '.',
            };
            if left_ok && right_ok {
                consider(i, mch.len(), Kind::Marker(norm_m));
                break;
            }
            from = i + 1;
            if from >= lower.len() {
                break;
            }
        }
    }
    for (pi, p) in PLACES.iter().enumerate() {
        let pch: Vec<char> = p.chars().collect();
        if let Some(i) = find_sub(&lower, &pch, 0) {
            let left_ok = i == 0 || !orig[i - 1].is_alphanumeric();
            let right_ok = match orig.get(i + pch.len()) {
                None => true,
                Some(&c) => !c.is_alphanumeric(),
            };
            if left_ok && right_ok {
                consider(i, pch.len(), Kind::Place(pi));
            }
        }
    }
    let (idx, len, kind) = match best {
        None => return (s.to_string(), String::new(), None),
        Some(b) => b,
    };
    let before = substr(&orig, 0, idx).trim().to_string();
    match kind {
        Kind::Place(pi) => {
            let after = substr(&orig, idx + len, orig.len()).trim().to_string();
            let mut name = substr(&orig, idx, idx + len).trim().to_string();
            if let Some(f) = name.chars().next() {
                if f.is_lowercase() {
                    let mut c = name.chars();
                    let f = c.next().unwrap();
                    name = f.to_uppercase().collect::<String>() + c.as_str();
                }
            }
            let _ = pi;
            let rest = match (before.is_empty(), after.is_empty()) {
                (true, true) => String::new(),
                (true, false) => after,
                (false, true) => before,
                (false, false) => format!("{before} {after}"),
            };
            (rest, name, None)
        }
        Kind::Marker(norm_m) => {
            let tail_str = substr(&orig, idx + len, orig.len());
            let mut tail = tail_str.trim_start();
            tail = tail.trim_start_matches(|c: char| c == '.' || c.is_whitespace());
            tail = tail
                .trim_start_matches(|c: char| c == '№' || c == '#' || c == 'N' || c == 'n')
                .trim_start();
            let num_end = tail
                .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '/' || c == '–'))
                .unwrap_or(tail.len());
            let num = tail[..num_end]
                .trim_matches(|c: char| c == ',' || c == ';' || c == '.')
                .to_string();
            if num.is_empty() || !num.chars().any(|c| c.is_ascii_digit()) {
                return (
                    before,
                    norm_m.to_string(),
                    Some(format!(
                        "маркер «{norm_m}» без номера — проверьте ячейку"
                    )),
                );
            }
            let after_num = tail[num_end..].trim();
            let after_num = after_num
                .trim_start_matches(|c: char| c == ',' || c == ';')
                .trim();
            let rest = match (before.is_empty(), after_num.is_empty()) {
                (true, true) => String::new(),
                (true, false) => after_num.to_string(),
                (false, true) => before,
                (false, false) => format!("{before} {after_num}"),
            };
            (rest, format!("{norm_m} {num}"), None)
        }
    }
}
fn is_single_initial(w: &str) -> bool {
    let t = w.trim_matches(|c: char| c == ',' || c == ';');
    if !t.contains('.') {
        return false;
    }
    let stripped: String = t.chars().filter(|c| *c != '.').collect();
    let n = stripped.chars().count();
    (n == 1 || n == 2) && stripped.chars().all(|c| c.is_uppercase())
}
fn is_combined_initials(w: &str) -> bool {
    let t = w.trim_matches(|c: char| c == ',' || c == ';');
    if !t.contains('.') {
        return false;
    }
    let stripped: String = t.chars().filter(|c| *c != '.').collect();
    stripped.chars().count() == 2 && stripped.chars().all(|c| c.is_uppercase())
}
fn is_capitalized_word(w: &str) -> bool {
    let t = w.trim_matches(|c: char| c == ',' || c == ';' || c == '.');
    if t.chars().count() < 2 {
        return false;
    }
    let mut c = t.chars();
    match c.next() {
        None => false,
        Some(f) => {
            if !f.is_uppercase() {
                return false;
            }
            let rest_ok = c.all(|x| x.is_lowercase() || x == '-');
            let has_lower = t.chars().any(|x| x.is_lowercase());
            rest_ok && has_lower && !t.chars().any(|x| x.is_numeric())
        }
    }
}
fn split_trailing_teacher(s: &str) -> Option<(String, String, bool, bool)> {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() < 2 {
        return None;
    }
    let n = words.len();
    fn clean(w: &str) -> &str {
        w.trim_matches([',', ';'])
    }
    if n >= 4
        && is_capitalized_word(clean(words[n - 3]))
        && is_single_initial(words[n - 2])
        && is_single_initial(words[n - 1])
    {
        let t = words[n - 3..].join(" ");
        let subj = words[..n - 3].join(" ");
        if !subj.trim().is_empty() {
            return Some((subj, t, false, false));
        }
    }
    if n >= 3
        && is_capitalized_word(clean(words[n - 2]))
        && is_combined_initials(words[n - 1])
    {
        let t = words[n - 2..].join(" ");
        let subj = words[..n - 2].join(" ");
        if !subj.trim().is_empty() {
            return Some((subj, t, false, false));
        }
    }
    if n >= 3
        && is_capitalized_word(clean(words[n - 2]))
        && is_single_initial(words[n - 1])
    {
        let t = words[n - 2..].join(" ");
        let subj = words[..n - 2].join(" ");
        if !subj.trim().is_empty() {
            return Some((subj, t, false, false));
        }
    }
    if n >= 4
        && is_capitalized_word(clean(words[n - 3]))
        && is_capitalized_word(clean(words[n - 2]))
        && is_capitalized_word(clean(words[n - 1]))
    {
        let subj = words[..n - 3].join(" ");
        if !subj.trim().is_empty() {
            let t = words[n - 3..].join(" ");
            if is_patronymic(words[n - 1])
                || (is_first_name(clean(words[n - 3]))
                    && (is_patronymic(words[n - 2]) || is_surname_like(words[n - 1])))
            {
                return Some((subj, t, true, false));
            }
            return Some((subj, String::new(), false, true));
        }
        return None;
    }
    if n >= 3
        && is_capitalized_word(clean(words[n - 2]))
        && is_capitalized_word(clean(words[n - 1]))
    {
        let subj = words[..n - 2].join(" ");
        if !subj.trim().is_empty() {
            let t = words[n - 2..].join(" ");
            let w1 = clean(words[n - 2]);
            let w2 = clean(words[n - 1]);
            if is_first_name(w1) || is_first_name(w2) {
                return Some((subj, t, true, false));
            }
            if is_surname_like(w1) || is_surname_like(w2) {
                return Some((subj, String::new(), false, true));
            }
        }
    }
    if n >= 2
        && is_capitalized_word(clean(words[n - 1]))
        && is_surname_like(words[n - 1])
        && !is_first_name(clean(words[n - 1]))
    {
        return Some((s.to_string(), String::new(), false, true));
    }
    None
}
fn is_first_name(w: &str) -> bool {
    const NAMES: [&str; 79] = [
        "александр", "александра", "алексей", "алёна", "алена", "анастасия", "андрей",
        "анна", "антон", "артём", "артем", "борис", "валентина", "валерия", "валерий",
        "василий", "вера", "вероника", "виктор", "виктория", "владимир", "владислав",
        "галина", "георгий", "григорий", "даниил", "данила", "дарья", "денис",
        "дмитрий", "екатерина", "елена", "елизавета", "жанна", "иван",
        "игорь", "илья", "инна", "ирина", "кирилл", "константин", "кристина",
        "ксения", "лариса", "лидия", "лука", "любовь", "людмила", "максим", "марина",
        "мария", "маргарита", "матвей", "михаил", "надежда", "наталья", "наталия",
        "никита", "николай", "нина", "олег", "ольга", "павел", "полина", "роман",
        "светлана", "семён", "семен", "сергей", "софия", "софья", "степан",
        "татьяна", "тимофей", "федор", "фёдор", "юлия", "юрий", "яна",
    ];
    let low = w.to_lowercase();
    NAMES.contains(&low.as_str())
}
fn is_patronymic(w: &str) -> bool {
    let t = w.trim_matches([',', ';', '.']).to_lowercase();
    ["ович", "евич", "ич", "овна", "евна", "ична", "инична"]
        .iter()
        .any(|s| t.ends_with(s) && t.len() > s.len())
}
fn is_surname_like(w: &str) -> bool {
    let t = w.trim_matches([',', ';', '.']).to_lowercase();
    ["овский", "евский", "инский", "ский", "цкий", "ской", "ской", "ов", "ев", "ёв",
        "ин", "ын", "ой", "ей", "ко", "ук", "юк", "ак", "як", "аго", "яго", "ых", "их"]
        .iter()
        .any(|s| t.ends_with(s) && t.len() > s.len() + 1)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parsed(raw: &str) -> (Option<Lesson>, Option<String>) {
        parse_lesson(raw)
    }
    #[test]
    fn classic_brackets_and_kab() {
        let (l, w) = parsed("Математика каб. 101 (Иванов И.И.)");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "Математика");
        assert_eq!(l.room, "каб. 101");
        assert_eq!(l.teacher, "Иванов И.И.");
        assert!(!l.full);
        assert!(w.is_none(), "warn: {w:?}");
    }
    #[test]
    fn place_only_is_full() {
        let (l, _) = parsed("Актовый зал");
        let l = l.expect("lesson");
        assert_eq!(l.room, "Актовый зал");
        assert!(l.subject.is_empty());
        assert!(l.full);
    }
    #[test]
    fn room_only_number_is_full() {
        let (l, _) = parsed("каб 101");
        let l = l.expect("lesson");
        assert_eq!(l.room, "каб. 101");
        assert!(l.full);
    }
    #[test]
    fn aud_without_dots_and_teacher_no_brackets() {
        let (l, w) = parsed("Физика ауд 205 Петров П.П.");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "Физика");
        assert_eq!(l.room, "ауд. 205");
        assert_eq!(l.teacher, "Петров П.П.");
        assert!(!l.full);
        assert!(w.is_none(), "warn: {w:?}");
    }
    #[test]
    fn lab_marker() {
        let (l, _) = parsed("Химия лаборатория 3 (Петрова А.)");
        let l = l.expect("lesson");
        assert_eq!(l.room, "лаб. 3");
        assert_eq!(l.teacher, "Петрова А.");
    }
    #[test]
    fn teacher_name_surname_guessed_with_warning() {
        let (l, w) = parsed("История Иван Петров");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "История");
        assert_eq!(l.teacher, "Иван Петров");
        assert!(w.is_some(), "ожидалось предупреждение об угадывании");
    }
    #[test]
    fn unclosed_bracket_warns_but_parses() {
        let (l, w) = parsed("Математика (Иванов И.И.");
        let l = l.expect("lesson");
        assert_eq!(l.teacher, "Иванов И.И.");
        assert_eq!(l.subject, "Математика");
        assert!(w.is_some());
    }
    #[test]
    fn kab_without_number_warns() {
        let (l, w) = parsed("Математика каб. (Иванов)");
        let l = l.expect("lesson");
        assert_eq!(l.room, "каб.");
        assert!(w.is_some());
    }
    #[test]
    fn tail_after_bracket_is_room() {
        let (l, _) = parsed("(Сидоров) Спортзал");
        let l = l.expect("lesson");
        assert_eq!(l.room, "Спортзал");
        assert_eq!(l.teacher, "Сидоров");
        assert!(l.full);
    }
    #[test]
    fn empty_cell_is_none() {
        let (l, _) = parsed("   ");
        assert!(l.is_none());
    }
    #[test]
    fn compound_subject_not_split() {
        let (l, w) = parsed("История России");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "История России");
        assert!(l.teacher.is_empty(), "teacher: {:?}", l.teacher);
        assert!(w.is_none(), "warn: {w:?}");
    }
    #[test]
    fn three_word_subject_not_split() {
        let (l, w) = parsed("История Древнего Мира");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "История Древнего Мира");
        assert!(l.teacher.is_empty());
        assert!(w.is_none(), "warn: {w:?}");
    }
    #[test]
    fn full_name_with_patronymic_guessed() {
        let (l, w) = parsed("Математика Иванов Иван Иванович");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "Математика");
        assert_eq!(l.teacher, "Иванов Иван Иванович");
        assert!(w.is_some());
    }
    #[test]
    fn surname_only_warns_not_splits() {
        let (l, w) = parsed("История Петров");
        let l = l.expect("lesson");
        assert_eq!(l.subject, "История Петров");
        assert!(l.teacher.is_empty());
        assert!(w.is_some());
    }
    #[test]
    fn kab_number_with_dash_and_slash() {
        let (l, w) = parsed("Физика каб. 101-а (Петров)");
        let l = l.expect("lesson");
        assert_eq!(l.room, "каб. 101-а");
        assert!(w.is_none(), "warn: {w:?}");
    }
}
