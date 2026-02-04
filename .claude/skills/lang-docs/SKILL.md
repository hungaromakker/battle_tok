# Language Documentation Search

Search and retrieve programming language documentation stored locally in `languages/`.

---

## Usage

```
/lang-docs <language> [topic]
```

**Examples:**
- `/lang-docs nextjs` - Overview of Next.js documentation
- `/lang-docs nextjs directives` - use client/server/cache directives
- `/lang-docs nextjs caching` - All 4 caching mechanisms
- `/lang-docs rust tokio` - Tokio async runtime docs
- `/lang-docs rust iter` - Iterator methods
- `/lang-docs python fastapi` - FastAPI documentation

---

## Quick Search Commands

Workers should use grep to search documentation without loading full files:

```bash
# See available documentation
cat languages/INDEX.md

# Search by topic (cross-language)
grep -r "TOPIC" languages/

# Language-specific searches
grep -r "TOPIC" languages/nextjs/
grep -r "TOPIC" languages/rust/
grep -r "TOPIC" languages/python/
```

---

## Available Documentation (26 files)

| Language | Files | Key Topics |
|----------|-------|------------|
| **Next.js 16.1.4** | 7 | App Router, PPR, caching, directives, Server Actions, glossary |
| **Rust 1.93.0** | 16 | std library, collections, iter, io, sync, tokio, serde, axum |
| **Python 3.11+** | 2 | FastAPI, async, type hints |
| **TypeScript 5.x** | 1 | Types, generics, utility types |

---

## Structure

```
languages/
├── INDEX.md                    # Master index with search examples
├── nextjs/
│   ├── overview.md             # Framework basics, PPR, Turbopack
│   ├── glossary.md             # Complete A-Z terminology
│   ├── directives.md           # "use client/server/cache"
│   ├── routing.md              # App Router, layouts, dynamic routes
│   ├── data-fetching.md        # Server/client fetching, streaming
│   ├── api-routes.md           # Route handlers, middleware/proxy
│   └── caching.md              # 4 caching mechanisms
├── rust/
│   ├── overview.md             # Language basics, cargo
│   ├── patterns/overview.md    # Builder, TypeState, RAII
│   └── crates/
│       ├── std/                # Standard library (7 files)
│       │   ├── overview.md
│       │   ├── collections.md  # Vec, HashMap, BTreeMap
│       │   ├── iter.md         # Iterator methods
│       │   ├── io.md           # Read, Write, BufReader
│       │   ├── fs.md           # File system ops
│       │   ├── thread.md       # spawn, scope, JoinHandle
│       │   └── sync.md         # Mutex, Arc, RwLock, channels
│       ├── core/overview.md    # no_std core library
│       ├── tokio/overview.md   # Async runtime
│       ├── serde/overview.md   # Serialization
│       ├── axum/overview.md    # Web framework
│       ├── clap/overview.md    # CLI parsing
│       ├── anyhow/overview.md  # Error handling
│       └── thiserror/overview.md
├── python/
│   ├── overview.md             # Language basics, venv, async
│   └── fastapi/overview.md     # FastAPI web framework
└── typescript/
    └── overview.md             # Types, generics, utility types
```

---

## Common Search Patterns

### Next.js
```bash
grep -r "use server\|use client\|use cache" languages/nextjs/   # Directives
grep -r "revalidate\|cacheLife\|cacheTag" languages/nextjs/     # Caching
grep -r "cookies\|headers\|searchParams" languages/nextjs/      # Dynamic APIs
grep -r "Suspense\|streaming\|PPR" languages/nextjs/            # Streaming/PPR
grep -r "Server Action\|Server Function" languages/nextjs/      # Mutations
```

### Rust
```bash
grep -r "HashMap\|Vec\|BTreeMap" languages/rust/                # Collections
grep -r "\.map\|\.filter\|\.fold" languages/rust/               # Iterators
grep -r "async.*await\|tokio" languages/rust/                   # Async
grep -r "Result\|Option\|anyhow\|thiserror" languages/rust/     # Errors
grep -r "Mutex\|Arc\|RwLock\|channel" languages/rust/           # Concurrency
grep -r "Serialize\|Deserialize" languages/rust/                # Serde
```

### Python
```bash
grep -r "@app\.\(get\|post\)" languages/python/                 # FastAPI routes
grep -r "BaseModel\|Pydantic" languages/python/                 # Validation
grep -r "async def\|await" languages/python/                    # Async
```

---

## Reading Full Files

When you need detailed examples, read the full doc:

```bash
cat languages/nextjs/glossary.md      # All Next.js terms
cat languages/nextjs/directives.md    # Directive details
cat languages/rust/crates/std/iter.md # All iterator methods
cat languages/rust/crates/tokio/overview.md
```

---

## Adding Documentation

When adding new language docs:

1. Create language folder: `languages/<lang>/`
2. Add `overview.md` with language basics
3. Create subfolders for frameworks/libraries
4. Update `languages/INDEX.md` with file count and structure
5. Keep docs concise - AI-optimized, not human tutorials

### Doc Format Guidelines

Each doc file should be:
- **Concise**: Key patterns, not tutorials
- **Code-heavy**: More examples, less prose
- **Searchable**: Clear headings for grep
- **AI-optimized**: Patterns workers can copy/adapt
