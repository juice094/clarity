package com.juice094.clarity.mobile.ui.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp

/**
 * Lightweight Markdown renderer for assistant messages.
 *
 * Supports:
 * - Headings (#, ##, ###)
 * - Bold (**text**) and italic (*text*)
 * - Inline code (`code`)
 * - Code blocks (```lang ... ```)
 * - Bullet lists (- item, * item)
 * - Links ([text](url))
 *
 * This is intentionally dependency-free and simple. Complex nested structures
 * are rendered gracefully as plain text when unsupported.
 */
@Composable
fun MarkdownText(content: String, modifier: Modifier = Modifier) {
    val blocks = remember(content) { parseMarkdownBlocks(content) }
    Column(modifier = modifier) {
        blocks.forEach { block ->
            MarkdownBlock(block = block)
        }
    }
}

internal sealed class MdBlock {
    data class Heading(val level: Int, val text: String) : MdBlock()
    data class Paragraph(val text: String) : MdBlock()
    data class BulletList(val items: List<String>) : MdBlock()
    data class CodeBlock(val language: String, val code: String) : MdBlock()
}

@Composable
private fun MarkdownBlock(block: MdBlock) {
    when (block) {
        is MdBlock.Heading -> {
            Text(
                text = parseInline(block.text),
                style = when (block.level) {
                    1 -> MaterialTheme.typography.headlineMedium
                    2 -> MaterialTheme.typography.headlineSmall
                    else -> MaterialTheme.typography.titleLarge
                },
                modifier = Modifier.padding(vertical = 4.dp)
            )
        }

        is MdBlock.Paragraph -> {
            Text(
                text = parseInline(block.text),
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.padding(vertical = 2.dp)
            )
        }

        is MdBlock.BulletList -> {
            Column(modifier = Modifier.padding(vertical = 2.dp)) {
                block.items.forEach { item ->
                    Row {
                        Text(
                            text = "• ",
                            style = MaterialTheme.typography.bodyLarge
                        )
                        Text(
                            text = parseInline(item),
                            style = MaterialTheme.typography.bodyLarge
                        )
                    }
                }
            }
        }

        is MdBlock.CodeBlock -> {
            Card(
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceVariant
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 4.dp)
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    if (block.language.isNotBlank()) {
                        Text(
                            text = block.language,
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(bottom = 4.dp)
                        )
                    }
                    Text(
                        text = block.code,
                        fontFamily = FontFamily.Monospace,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

internal fun parseMarkdownBlocks(content: String): List<MdBlock> {
    val lines = content.lines()
    val blocks = mutableListOf<MdBlock>()
    var i = 0

    while (i < lines.size) {
        val line = lines[i]

        when {
            line.startsWith("```") -> {
                val language = line.trimStart('`').trim()
                val code = mutableListOf<String>()
                i++
                while (i < lines.size && !lines[i].startsWith("```")) {
                    code.add(lines[i])
                    i++
                }
                // Skip the closing fence if present.
                if (i < lines.size) i++
                blocks.add(MdBlock.CodeBlock(language, code.joinToString("\n")))
            }

            line.startsWith("#") -> {
                val level = line.takeWhile { it == '#' }.length.coerceAtMost(6)
                val text = line.drop(level).trim()
                blocks.add(MdBlock.Heading(level, text))
                i++
            }

            line.startsWith("- ") || line.startsWith("* ") -> {
                val items = mutableListOf<String>()
                while (i < lines.size &&
                    (lines[i].startsWith("- ") || lines[i].startsWith("* "))
                ) {
                    items.add(lines[i].drop(2))
                    i++
                }
                blocks.add(MdBlock.BulletList(items))
            }

            line.isBlank() -> {
                i++
            }

            else -> {
                val paragraph = mutableListOf<String>()
                while (i < lines.size &&
                    lines[i].isNotBlank() &&
                    !lines[i].startsWith("#") &&
                    !lines[i].startsWith("```") &&
                    !lines[i].startsWith("- ") &&
                    !lines[i].startsWith("* ")
                ) {
                    paragraph.add(lines[i])
                    i++
                }
                blocks.add(MdBlock.Paragraph(paragraph.joinToString(" ")))
            }
        }
    }

    return blocks
}

@Composable
private fun parseInline(text: String): AnnotatedString = buildAnnotatedString {
    val patterns = listOf(
        InlinePattern(
            regex = """`([^`\n]+)`""".toRegex(),
            style = SpanStyle(
                fontFamily = FontFamily.Monospace,
                background = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
            )
        ),
        InlinePattern(
            regex = """\*\*\*(.+?)\*\*\*""".toRegex(),
            style = SpanStyle(fontWeight = FontWeight.Bold, fontStyle = FontStyle.Italic)
        ),
        InlinePattern(
            regex = """\*\*(.+?)\*\*""".toRegex(),
            style = SpanStyle(fontWeight = FontWeight.Bold)
        ),
        InlinePattern(
            regex = """\*(.+?)\*""".toRegex(),
            style = SpanStyle(fontStyle = FontStyle.Italic)
        ),
        InlinePattern(
            regex = """_([^_\n]+)_""".toRegex(),
            style = SpanStyle(fontStyle = FontStyle.Italic)
        ),
        InlinePattern(
            regex = """\[([^\]]+)\]\(([^)\n]+)\)""".toRegex(),
            style = SpanStyle(
                color = MaterialTheme.colorScheme.primary,
                textDecoration = TextDecoration.Underline
            ),
            contentGroup = 1
        )
    )

    var cursor = 0
    while (cursor < text.length) {
        val candidates = patterns.mapNotNull { pattern ->
            pattern.regex.find(text, cursor)?.let { pattern to it }
        }

        if (candidates.isEmpty()) {
            append(text.substring(cursor))
            break
        }

        // Prefer the earliest match; tie-break by longer delimiter.
        val (pattern, match) = candidates.minWith(
            compareBy<Pair<InlinePattern, MatchResult>> { it.second.range.first }
                .thenByDescending { it.second.range.last - it.second.range.first }
        )

        if (match.range.first > cursor) {
            append(text.substring(cursor, match.range.first))
        }

        val content = match.groupValues[pattern.contentGroup]
        pushStyle(pattern.style)
        append(content)
        pop()

        cursor = match.range.last + 1
    }
}

private data class InlinePattern(
    val regex: Regex,
    val style: SpanStyle,
    val contentGroup: Int = 1
)
