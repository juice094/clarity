#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)]

use clarity_memory::bm25::Bm25Index;
use clarity_memory::embedding::TfidfVectorizer;
use criterion::{Criterion, criterion_group, criterion_main};

// ── SparseVector::cosine_similarity ─────────────────────────────────────

fn bench_cosine_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("SparseVector::cosine_similarity");

    // Build vectors via TfidfVectorizer so we exercise the real data structure.
    let vocab: Vec<String> = (0..500).map(|i| format!("term_{}", i)).collect();
    let mut v = TfidfVectorizer::new();
    v.fit(&vocab);

    let doc_a = (0..10)
        .map(|i| format!("term_{}", i))
        .collect::<Vec<_>>()
        .join(" ");
    let doc_b = (0..10)
        .map(|i| format!("term_{}", i + 5))
        .collect::<Vec<_>>()
        .join(" ");
    let doc_c = (400..410)
        .map(|i| format!("term_{}", i))
        .collect::<Vec<_>>()
        .join(" ");

    let sv_a = v.transform(&doc_a);
    let sv_b = v.transform(&doc_b);
    let sv_c = v.transform(&doc_c);

    group.bench_function("overlapping (50%)", |bench| {
        bench.iter(|| sv_a.cosine_similarity(&sv_b));
    });
    group.bench_function("disjoint", |bench| {
        bench.iter(|| sv_a.cosine_similarity(&sv_c));
    });
    group.finish();
}

// ── TfidfVectorizer::transform ──────────────────────────────────────────

fn bench_tfidf_transform(c: &mut Criterion) {
    let mut group = c.benchmark_group("TfidfVectorizer::transform");

    // Small vocab, short doc.
    let small_vocab: Vec<String> = (0..100).map(|i| format!("term_{}", i)).collect();
    let mut vectorizer = TfidfVectorizer::new();
    vectorizer.fit(&small_vocab);
    let short_doc = "term_0 term_5 term_10 term_20 term_50";
    group.bench_function("small_vocab(100)_short_doc", |bench| {
        bench.iter(|| vectorizer.transform(short_doc));
    });

    // Larger doc.
    let long_doc = (0..20)
        .map(|i| format!("term_{}", i * 2))
        .collect::<Vec<_>>()
        .join(" ");
    group.bench_function("small_vocab(100)_long_doc", |bench| {
        bench.iter(|| vectorizer.transform(&long_doc));
    });
    group.finish();
}

// ── Bm25Index::score ────────────────────────────────────────────────────

fn bench_bm25_score(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bm25Index::score");

    // Build a corpus of 500 docs to get meaningful IDF values.
    let doc_count = 500;
    let docs: Vec<String> = (0..doc_count)
        .map(|i| {
            (0..(10 + i % 20))
                .map(|j| match j % 4 {
                    0 => "the",
                    1 => "rust",
                    2 => "memory",
                    _ => "search",
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let idx = Bm25Index::new(&docs);

    // Score doc 0 against queries.
    group.bench_function("corpus_500_query_short", |bench| {
        bench.iter(|| idx.score("rust memory", 0));
    });

    group.bench_function("corpus_500_query_long", |bench| {
        bench.iter(|| idx.score("rust memory search the index", 0));
    });

    // Also benchmark the full search pipeline.
    group.bench_function("corpus_500_search_top10", |bench| {
        bench.iter(|| idx.search("rust memory", 10));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_cosine_similarity,
    bench_tfidf_transform,
    bench_bm25_score,
);
criterion_main!(benches);
