use super::*;
use crate::utils::{log_error, log_info};
use ort::session::{builder::GraphOptimizationLevel, Session};
use tauri::Emitter;
use tokenizers::Tokenizer;
use tokio::time::{timeout, Duration};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    success: bool,
    message: String,
    model_info: ModelTestInfo,
    health: HealthCheck,
    retrieval: RetrievalCheck,
    separation: SeparationCheck,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelTestInfo {
    version: String,
    max_tokens: u32,
    embedding_dimensions: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthCheck {
    load_ok: bool,
    identity_cosine: f32,
    passed: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalCheck {
    case_count: usize,
    top1_rate: f32,
    top3_rate: f32,
    mrr: f32,
    passed: bool,
    cases: Vec<RetrievalCaseResult>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RetrievalCaseResult {
    name: String,
    query: String,
    expected_id: String,
    expected_text: String,
    rank: u32,
    correct: bool,
    top_id: String,
    top_text: String,
    top_score: f32,
    correct_score: f32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SeparationCheck {
    related_avg: f32,
    unrelated_avg: f32,
    margin: f32,
    passed: bool,
    related_pairs: Vec<NamedScore>,
    unrelated_pairs: Vec<NamedScore>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NamedScore {
    name: String,
    text_a: String,
    text_b: String,
    score: f32,
}

const IDENTITY_PROBE: &str =
    "The quick brown fox jumps over the lazy dog and lands beside the riverbank.";
const IDENTITY_PASS_THRESHOLD: f32 = 0.9990;
const TOP1_PASS_THRESHOLD: f32 = 0.60;
const SEPARATION_MARGIN_PASS_THRESHOLD: f32 = 0.10;

fn retrieval_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        // Hidden items / clues
        ("tea-tin-key", "Mirelle hid the brass key inside the tea tin with a false bottom in the pantry."),
        ("pier-nine", "Dockworkers moved the contraband crate to Pier Nine just before the storm reached the harbor."),
        ("glasshouse", "A rain-stained letter says the secret meeting will happen in the glasshouse behind the manor gardens."),
        ("engine-resin", "Pressure valves in the engine room were jammed shut with hardened resin, forcing the airship to descend."),
        ("signet-pears", "The cook insists the missing signet ring was last seen beside a bowl of pears in the kitchen."),
        ("watch-shift", "The watch captain changed shifts at the clocktower exactly three minutes before midnight."),
        ("crystal-cave", "Beyond the waterfall, a crystal cave glows with pale blue light and echoes every footstep."),
        ("burnt-map", "Half the map was burned in the fire, but you could still make out the trail to the old chapel."),
        ("locket-portrait", "Inside the silver locket was a tiny portrait of a woman with a crescent-moon birthmark."),
        ("library-cipher", "A handwritten cipher tucked between two old astronomy books decoded into a single street name."),
        ("attic-trunk", "An old leather trunk in the attic held love letters tied together with a faded green ribbon."),
        ("garden-coin", "A foreign coin with a swan on it was buried beneath the third rose bush in the kitchen garden."),
        ("forge-blueprint", "Folded inside the blacksmith's apron pocket was a blueprint for a one-handed crossbow."),
        // Character promises and intentions
        ("lyra-promise", "Lyra promised to come back to the lighthouse by autumn, no matter what happened in the capital."),
        ("kael-revenge", "Kael swore on his father's grave that he would find every man who burned the village."),
        ("isolde-marriage", "Isolde refused the prince's proposal and said she would rather work in a bakery for the rest of her life."),
        ("merrick-debt", "Merrick promised to repay the gambling debt before the festival, even if it meant selling his horse."),
        ("rhys-letter", "Rhys said he would write a letter every Sunday, even from the front lines."),
        ("aelin-secret", "Aelin asked you to keep the truth about her brother hidden until winter solstice."),
        ("captain-mutiny", "The captain warned the first mate that one more incident would mean stripping his rank in port."),
        // Locations and settings
        ("northern-keep", "The Northern Keep is built into the side of a cliff, accessible only by a narrow switchback path."),
        ("rust-quarter", "The Rust Quarter is the oldest part of the city, where every building leans slightly to the east."),
        ("twin-moons-inn", "The Twin Moons Inn sits at the crossroads where three trade routes meet."),
        ("salt-flats", "The salt flats stretch for miles past the abandoned mining town, white and shimmering at noon."),
        ("blackwater-pond", "Blackwater Pond is rumored to swallow anything dropped into it without making a single ripple."),
        ("orchard-shrine", "Behind the apple orchard is a small stone shrine no one visits except on the autumn equinox."),
        ("sunken-cathedral", "Half the cathedral has sunk into the river over the past century, and now the bell tower is underwater."),
        // Plot events and discoveries
        ("courier-ambush", "The courier was ambushed two days before reaching the city, and the diplomatic seal was stolen."),
        ("council-vote", "The council voted six to four in favor of opening the trade route, but the decision was not announced."),
        ("plague-arrival", "The first signs of the plague appeared in the riverside districts the morning after the festival."),
        ("astronomer-discovery", "The court astronomer discovered a new comet but was forbidden from publishing the finding."),
        ("treaty-signed", "The treaty was signed at the marble pavilion at dawn, witnessed only by the two spymasters."),
        ("prince-disappeared", "The young prince vanished from his rooms during the masquerade and has not been seen since."),
        ("forge-explosion", "The royal forge exploded the night before the gala, killing two apprentices and a senior smith."),
        // Lore / world facts
        ("wolf-pact", "The northern clan signed a quiet pact with the wolves of the Greywood after the second winter."),
        ("frost-festival", "Every twelve years the city throws a Frost Festival to commemorate surviving the long winter of legend."),
        ("guild-rule", "By guild law, no apprentice may forge a named blade until they have served seven full years."),
        ("desert-rite", "The desert tribes consider it sacred to share salt and water with anyone who arrives at their fire."),
        ("old-tongue", "The Old Tongue is no longer spoken outside of academic ceremonies and a single mountain village."),
        ("twin-stars", "Twin stars appear in the southern sky once every hundred years and are said to mark the birth of a true heir."),
        ("ironwood-treaty", "The ironwood forests are jointly held by three kingdoms under a treaty older than any living memory."),
        // Characters and relationships
        ("eira-rival", "Eira and the captain of the second regiment have been rivals since their academy days."),
        ("tomen-mentor", "Tomen took the orphaned scribe under his wing after finding the boy reading by candlelight in a stable."),
        ("dancer-spy", "The dancer at the silk pavilion is actually a spy for the eastern duchy."),
        ("blacksmith-deaf", "The old blacksmith has been completely deaf since the forge explosion and reads lips."),
        ("priest-doubts", "The high priest privately doubts the new prophecy but performs the rites anyway."),
        ("twin-brothers", "The twin brothers were separated at birth and only reunited last spring at a horse fair."),
        ("widow-archer", "The widow at the edge of the village was the most decorated archer of the previous war."),
        // Scenes and atmosphere
        ("rain-funeral", "It rained the entire day of the funeral, and only six people stood at the graveside."),
        ("market-fire", "The morning market burned down on the day of the eclipse, taking half the eastern stalls with it."),
    ]
}

struct RetrievalCase {
    name: &'static str,
    query: &'static str,
    expected_id: &'static str,
}

fn retrieval_cases() -> Vec<RetrievalCase> {
    vec![
        RetrievalCase {
            name: "Brass key",
            query: "Where did Mirelle hide the brass key?",
            expected_id: "tea-tin-key",
        },
        RetrievalCase {
            name: "Crate pier",
            query: "Which pier did the contraband crate end up at before the storm?",
            expected_id: "pier-nine",
        },
        RetrievalCase {
            name: "Meeting place",
            query: "Where is the secret meeting going to happen?",
            expected_id: "glasshouse",
        },
        RetrievalCase {
            name: "Airship descent",
            query: "Why did the airship have to come down?",
            expected_id: "engine-resin",
        },
        RetrievalCase {
            name: "Signet ring",
            query: "Where was the missing signet ring last seen?",
            expected_id: "signet-pears",
        },
        RetrievalCase {
            name: "Watch shifts",
            query: "When does the watch captain switch shifts at the clocktower?",
            expected_id: "watch-shift",
        },
        RetrievalCase {
            name: "Behind waterfall",
            query: "What is hidden behind the waterfall?",
            expected_id: "crystal-cave",
        },
        RetrievalCase {
            name: "Burned map",
            query: "What did the burned map still show?",
            expected_id: "burnt-map",
        },
        RetrievalCase {
            name: "Locket contents",
            query: "What was inside the silver locket?",
            expected_id: "locket-portrait",
        },
        RetrievalCase {
            name: "Astronomy books",
            query: "What was tucked between the old astronomy books?",
            expected_id: "library-cipher",
        },
        RetrievalCase {
            name: "Attic trunk",
            query: "What was inside the leather trunk in the attic?",
            expected_id: "attic-trunk",
        },
        RetrievalCase {
            name: "Buried coin",
            query: "What was buried beneath the third rose bush?",
            expected_id: "garden-coin",
        },
        RetrievalCase {
            name: "Apron blueprint",
            query: "What was folded in the blacksmith's apron?",
            expected_id: "forge-blueprint",
        },
        RetrievalCase {
            name: "Lyra return",
            query: "When did Lyra say she would come back to the lighthouse?",
            expected_id: "lyra-promise",
        },
        RetrievalCase {
            name: "Kael oath",
            query: "What did Kael swear on his father's grave?",
            expected_id: "kael-revenge",
        },
        RetrievalCase {
            name: "Isolde refusal",
            query: "What did Isolde say when she refused the prince?",
            expected_id: "isolde-marriage",
        },
        RetrievalCase {
            name: "Merrick debt",
            query: "How is Merrick going to pay back his gambling debt?",
            expected_id: "merrick-debt",
        },
        RetrievalCase {
            name: "Rhys letters",
            query: "How often did Rhys promise to write?",
            expected_id: "rhys-letter",
        },
        RetrievalCase {
            name: "Aelin secret",
            query: "Until when did Aelin ask you to keep her brother's truth hidden?",
            expected_id: "aelin-secret",
        },
        RetrievalCase {
            name: "Captain warning",
            query: "What did the captain warn the first mate would happen?",
            expected_id: "captain-mutiny",
        },
        RetrievalCase {
            name: "Northern Keep access",
            query: "How do you actually reach the Northern Keep?",
            expected_id: "northern-keep",
        },
        RetrievalCase {
            name: "Rust Quarter detail",
            query: "What is unusual about the buildings in the Rust Quarter?",
            expected_id: "rust-quarter",
        },
        RetrievalCase {
            name: "Twin Moons Inn",
            query: "Where is the Twin Moons Inn located?",
            expected_id: "twin-moons-inn",
        },
        RetrievalCase {
            name: "Mining town",
            query: "What is past the abandoned mining town?",
            expected_id: "salt-flats",
        },
        RetrievalCase {
            name: "Blackwater rumor",
            query: "What is the rumor about Blackwater Pond?",
            expected_id: "blackwater-pond",
        },
        RetrievalCase {
            name: "Orchard shrine",
            query: "When do people visit the small shrine behind the apple orchard?",
            expected_id: "orchard-shrine",
        },
        RetrievalCase {
            name: "Cathedral fate",
            query: "What happened to the old cathedral?",
            expected_id: "sunken-cathedral",
        },
        RetrievalCase {
            name: "Courier ambush",
            query: "What happened to the courier on the way to the city?",
            expected_id: "courier-ambush",
        },
        RetrievalCase {
            name: "Trade route vote",
            query: "How did the council vote on opening the trade route?",
            expected_id: "council-vote",
        },
        RetrievalCase {
            name: "Plague onset",
            query: "When did the plague first appear in the city?",
            expected_id: "plague-arrival",
        },
        RetrievalCase {
            name: "Astronomer find",
            query: "What did the court astronomer discover?",
            expected_id: "astronomer-discovery",
        },
        RetrievalCase {
            name: "Treaty signing",
            query: "Where was the treaty signed and by whom?",
            expected_id: "treaty-signed",
        },
        RetrievalCase {
            name: "Prince vanishing",
            query: "When did the young prince disappear?",
            expected_id: "prince-disappeared",
        },
        RetrievalCase {
            name: "Forge incident",
            query: "What happened at the royal forge before the gala?",
            expected_id: "forge-explosion",
        },
        RetrievalCase {
            name: "Wolf pact",
            query: "Who did the northern clan make a pact with?",
            expected_id: "wolf-pact",
        },
        RetrievalCase {
            name: "Frost Festival",
            query: "How often is the Frost Festival held?",
            expected_id: "frost-festival",
        },
        RetrievalCase {
            name: "Named blades rule",
            query: "What is the guild rule about forging named blades?",
            expected_id: "guild-rule",
        },
        RetrievalCase {
            name: "Desert hospitality",
            query: "What do the desert tribes consider sacred?",
            expected_id: "desert-rite",
        },
        RetrievalCase {
            name: "Old Tongue",
            query: "Where is the Old Tongue still spoken today?",
            expected_id: "old-tongue",
        },
        RetrievalCase {
            name: "Twin stars meaning",
            query: "What do the twin stars in the southern sky signify?",
            expected_id: "twin-stars",
        },
        RetrievalCase {
            name: "Ironwood ownership",
            query: "Who controls the ironwood forests?",
            expected_id: "ironwood-treaty",
        },
        RetrievalCase {
            name: "Eira rivalry",
            query: "Who is Eira's long-standing rival?",
            expected_id: "eira-rival",
        },
        RetrievalCase {
            name: "Tomen apprentice",
            query: "How did Tomen find the orphaned scribe?",
            expected_id: "tomen-mentor",
        },
        RetrievalCase {
            name: "Silk pavilion dancer",
            query: "Who is the dancer at the silk pavilion really working for?",
            expected_id: "dancer-spy",
        },
        RetrievalCase {
            name: "Blacksmith hearing",
            query: "Why is the old blacksmith deaf?",
            expected_id: "blacksmith-deaf",
        },
        RetrievalCase {
            name: "Priest doubt",
            query: "What does the high priest privately doubt?",
            expected_id: "priest-doubts",
        },
        RetrievalCase {
            name: "Twin reunion",
            query: "When did the twin brothers find each other again?",
            expected_id: "twin-brothers",
        },
        RetrievalCase {
            name: "Famed archer",
            query: "Who was the most decorated archer of the previous war?",
            expected_id: "widow-archer",
        },
        RetrievalCase {
            name: "Funeral attendance",
            query: "How many people came to the funeral?",
            expected_id: "rain-funeral",
        },
        RetrievalCase {
            name: "Eclipse market",
            query: "What burned down on the day of the eclipse?",
            expected_id: "market-fire",
        },
    ]
}

struct PairDef {
    name: &'static str,
    a: &'static str,
    b: &'static str,
}

fn related_pairs() -> Vec<PairDef> {
    vec![
        PairDef {
            name: "Emotional state",
            a: "She felt a wave of sadness wash over her.",
            b: "Her heart ached with sorrow and grief.",
        },
        PairDef {
            name: "Action description",
            a: "He drew his sword and charged at the enemy.",
            b: "The warrior unsheathed his blade and rushed forward to attack.",
        },
        PairDef {
            name: "Tavern setting",
            a: "The tavern was dimly lit with flickering candles.",
            b: "Candlelight cast shadows across the dark inn.",
        },
        PairDef {
            name: "Memory clue",
            a: "The brass key was hidden in the tea tin beneath the false bottom.",
            b: "Mirelle concealed the brass key inside a tea tin with a false base.",
        },
    ]
}

fn unrelated_pairs() -> Vec<PairDef> {
    vec![
        PairDef {
            name: "Fox vs physics",
            a: "The quick brown fox jumps over the lazy dog.",
            b: "Quantum mechanics describes subatomic particle behavior.",
        },
        PairDef {
            name: "Pizza vs computer",
            a: "I love eating pizza with extra cheese.",
            b: "The computer crashed and lost all my files.",
        },
        PairDef {
            name: "Harbor vs kitchen",
            a: "Dockworkers moved the contraband crate to Pier Nine before the storm.",
            b: "The missing ring was last seen beside a bowl of pears in the kitchen.",
        },
        PairDef {
            name: "Lighthouse vs wolves",
            a: "Lyra promised to come back to the lighthouse by autumn.",
            b: "The northern clan signed a quiet pact with the wolves of the Greywood.",
        },
    ]
}

pub async fn run_embedding_test(app: AppHandle) -> Result<TestResult, String> {
    log_info(&app, "embedding_test", "starting embedding test");

    let app_for_test = app.clone();
    super::ort_runtime::ensure_ort_init(&app_for_test).await?;

    let _ = app.emit(
        "embedding_test_progress",
        serde_json::json!({
            "current": 0,
            "total": 3,
            "stage": "starting"
        }),
    );

    let test_future = tokio::task::spawn_blocking(move || -> Result<TestResult, String> {
        let detected_version = detect_model_version(&app_for_test)?;
        log_info(
            &app_for_test,
            "embedding_test",
            format!("detected model version {:?}", detected_version),
        );

        let active_config = resolve_runtime_model(&app_for_test)?;

        let mut session = Session::builder()
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to create session builder: {}", e),
                )
            })?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!("Failed to set optimization level: {}", e),
                )
            })?
            .commit_from_file(&active_config.model_path)
            .map_err(|e| {
                crate::utils::err_msg(
                    module_path!(),
                    line!(),
                    format!(
                        "Failed to load {} model: {}",
                        active_config.version_label, e
                    ),
                )
            })?;

        let tokenizer = Tokenizer::from_file(&active_config.tokenizer_path).map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to load tokenizer: {}", e),
            )
        })?;

        let mut embed = |text: &str| -> Result<Vec<f32>, String> {
            super::inference::compute_embedding_with_session(
                &mut session,
                &tokenizer,
                text,
                active_config.max_seq_length,
                active_config.embedding_dimensions,
            )
            .map_err(|e| {
                log_error(
                    &app_for_test,
                    "embedding_test",
                    format!("embed failure text_len={} error={}", text.len(), e),
                );
                e
            })
        };

        // 1. Health: identity test
        let _ = app_for_test.emit(
            "embedding_test_progress",
            serde_json::json!({
                "current": 1,
                "total": 3,
                "stage": "health"
            }),
        );

        let id_a = embed(IDENTITY_PROBE)?;
        let id_b = embed(IDENTITY_PROBE)?;
        let identity_cosine = super::util::cosine_similarity(&id_a, &id_b);
        let embedding_dim = id_a.len();
        let health_passed = identity_cosine >= IDENTITY_PASS_THRESHOLD;
        let health = HealthCheck {
            load_ok: true,
            identity_cosine,
            passed: health_passed,
        };
        log_info(
            &app_for_test,
            "embedding_test",
            format!(
                "health identity_cosine={} passed={}",
                identity_cosine, health_passed
            ),
        );

        // 2. Retrieval
        let _ = app_for_test.emit(
            "embedding_test_progress",
            serde_json::json!({
                "current": 2,
                "total": 3,
                "stage": "retrieval"
            }),
        );

        let docs = retrieval_corpus();
        let mut doc_embeddings: Vec<(&str, &str, Vec<f32>)> = Vec::with_capacity(docs.len());
        for (id, text) in &docs {
            let v = embed(text)?;
            doc_embeddings.push((*id, *text, v));
        }

        let cases = retrieval_cases();
        let case_count = cases.len();
        let mut case_results = Vec::with_capacity(case_count);
        let mut top1_hits = 0usize;
        let mut top3_hits = 0usize;
        let mut reciprocal_rank_sum = 0.0f32;

        for case in &cases {
            let q_emb = embed(case.query)?;
            let mut scored: Vec<(usize, f32)> = doc_embeddings
                .iter()
                .enumerate()
                .map(|(idx, (_id, _text, doc_emb))| {
                    (idx, super::util::cosine_similarity(&q_emb, doc_emb))
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let (top_idx, top_score) = scored[0];
            let (top_id, top_text, _) = doc_embeddings[top_idx];

            let expected_pos = doc_embeddings
                .iter()
                .position(|(id, _, _)| *id == case.expected_id);
            let (rank, correct, expected_text, correct_score) = match expected_pos {
                Some(expected_idx) => {
                    let rank_pos = scored
                        .iter()
                        .position(|(idx, _)| *idx == expected_idx)
                        .map(|p| p + 1)
                        .unwrap_or(0);
                    let correct = rank_pos == 1;
                    let exp_text = doc_embeddings[expected_idx].1;
                    let exp_score = scored
                        .iter()
                        .find(|(idx, _)| *idx == expected_idx)
                        .map(|(_, s)| *s)
                        .unwrap_or(0.0);
                    (rank_pos as u32, correct, exp_text, exp_score)
                }
                None => (0, false, "", 0.0),
            };

            if correct {
                top1_hits += 1;
            }
            if rank > 0 && rank <= 3 {
                top3_hits += 1;
            }
            if rank > 0 {
                reciprocal_rank_sum += 1.0 / rank as f32;
            }

            case_results.push(RetrievalCaseResult {
                name: case.name.to_string(),
                query: case.query.to_string(),
                expected_id: case.expected_id.to_string(),
                expected_text: expected_text.to_string(),
                rank,
                correct,
                top_id: top_id.to_string(),
                top_text: top_text.to_string(),
                top_score,
                correct_score,
            });
        }

        let case_count_f = case_count.max(1) as f32;
        let top1_rate = top1_hits as f32 / case_count_f;
        let top3_rate = top3_hits as f32 / case_count_f;
        let mrr = reciprocal_rank_sum / case_count_f;
        let retrieval_passed = top1_rate >= TOP1_PASS_THRESHOLD;
        let retrieval = RetrievalCheck {
            case_count,
            top1_rate,
            top3_rate,
            mrr,
            passed: retrieval_passed,
            cases: case_results,
        };
        log_info(
            &app_for_test,
            "embedding_test",
            format!(
                "retrieval top1={} top3={} mrr={} passed={}",
                top1_rate, top3_rate, mrr, retrieval_passed
            ),
        );

        // 3. Separation
        let _ = app_for_test.emit(
            "embedding_test_progress",
            serde_json::json!({
                "current": 3,
                "total": 3,
                "stage": "separation"
            }),
        );

        let related = related_pairs();
        let unrelated = unrelated_pairs();
        let mut related_scores = Vec::with_capacity(related.len());
        let mut unrelated_scores = Vec::with_capacity(unrelated.len());

        for pair in &related {
            let a = embed(pair.a)?;
            let b = embed(pair.b)?;
            let s = super::util::cosine_similarity(&a, &b);
            related_scores.push(NamedScore {
                name: pair.name.to_string(),
                text_a: pair.a.to_string(),
                text_b: pair.b.to_string(),
                score: s,
            });
        }
        for pair in &unrelated {
            let a = embed(pair.a)?;
            let b = embed(pair.b)?;
            let s = super::util::cosine_similarity(&a, &b);
            unrelated_scores.push(NamedScore {
                name: pair.name.to_string(),
                text_a: pair.a.to_string(),
                text_b: pair.b.to_string(),
                score: s,
            });
        }

        let related_avg = if related_scores.is_empty() {
            0.0
        } else {
            related_scores.iter().map(|s| s.score).sum::<f32>() / related_scores.len() as f32
        };
        let unrelated_avg = if unrelated_scores.is_empty() {
            0.0
        } else {
            unrelated_scores.iter().map(|s| s.score).sum::<f32>() / unrelated_scores.len() as f32
        };
        let margin = related_avg - unrelated_avg;
        let separation_passed = margin >= SEPARATION_MARGIN_PASS_THRESHOLD;
        let separation = SeparationCheck {
            related_avg,
            unrelated_avg,
            margin,
            passed: separation_passed,
            related_pairs: related_scores,
            unrelated_pairs: unrelated_scores,
        };
        log_info(
            &app_for_test,
            "embedding_test",
            format!(
                "separation related={} unrelated={} margin={} passed={}",
                related_avg, unrelated_avg, margin, separation_passed
            ),
        );

        let model_info = get_embedding_model_info(app_for_test.clone())?;
        let success = health_passed && retrieval_passed && separation_passed;

        let message = if success {
            "Model is healthy: identity stable, retrieval ranks correctly, related and unrelated text separate cleanly.".to_string()
        } else {
            let mut reasons: Vec<&str> = Vec::new();
            if !health_passed {
                reasons.push("identity check below threshold (model load may be corrupted)");
            }
            if !retrieval_passed {
                reasons.push("retrieval top-1 below floor");
            }
            if !separation_passed {
                reasons.push("separation margin too small");
            }
            format!("Issues: {}", reasons.join("; "))
        };

        Ok(TestResult {
            success,
            message,
            model_info: ModelTestInfo {
                version: model_info
                    .source_version
                    .or(model_info.version)
                    .unwrap_or_else(|| "unknown".to_string()),
                max_tokens: model_info.max_tokens,
                embedding_dimensions: embedding_dim,
            },
            health,
            retrieval,
            separation,
        })
    });

    let result = timeout(
        Duration::from_secs(EMBEDDING_TEST_TIMEOUT_SECS),
        test_future,
    )
    .await
    .map_err(|_| "Embedding test timed out. Please try again.".to_string())?
    .map_err(|e| {
        crate::utils::err_msg(
            module_path!(),
            line!(),
            format!("Embedding test failed to start: {}", e),
        )
    })?;

    let _ = app.emit(
        "embedding_test_progress",
        serde_json::json!({
            "current": 3,
            "total": 3,
            "stage": "completed"
        }),
    );

    result
}

pub async fn compare_custom_texts(
    app: AppHandle,
    text_a: String,
    text_b: String,
) -> Result<f32, String> {
    if text_a.trim().is_empty() || text_b.trim().is_empty() {
        return Err(crate::utils::err_msg(
            module_path!(),
            line!(),
            "Both texts must be non-empty",
        ));
    }

    log_info(
        &app,
        "embedding_test",
        format!(
            "compare custom texts len_a={} len_b={}",
            text_a.len(),
            text_b.len()
        ),
    );

    let emb_a = super::inference::compute_embedding(app.clone(), text_a)
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to embed first text: {}", e),
            )
        })?;

    let emb_b = super::inference::compute_embedding(app.clone(), text_b)
        .await
        .map_err(|e| {
            crate::utils::err_msg(
                module_path!(),
                line!(),
                format!("Failed to embed second text: {}", e),
            )
        })?;

    Ok(super::util::cosine_similarity(&emb_a, &emb_b))
}
