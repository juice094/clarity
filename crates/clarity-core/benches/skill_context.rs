use clarity_core::skills::SkillLoader;
use criterion::{Criterion, criterion_group, criterion_main};

const SKILL_MD: &str = r#"---
id: bench-skill
name: Bench Skill
description: A benchmark skill
tools:
  - bash
  - file_read
---

# Instructions
Do something useful.
"#;

fn bench_skill_parse(c: &mut Criterion) {
    c.bench_function("SkillLoader::parse", |b| {
        b.iter(|| {
            let _ = SkillLoader::parse(SKILL_MD);
        })
    });
}

fn bench_skill_build_context(c: &mut Criterion) {
    let skill = SkillLoader::parse(SKILL_MD).unwrap();
    c.bench_function("Skill::build_context", |b| {
        b.iter(|| {
            let _ = skill.build_context();
        })
    });
}

criterion_group!(benches, bench_skill_parse, bench_skill_build_context);
criterion_main!(benches);
