package com.juice094.clarity.mobile

import com.juice094.clarity.mobile.ui.components.MdBlock
import com.juice094.clarity.mobile.ui.components.parseMarkdownBlocks
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class MarkdownBlockParserTest {

    @Test
    fun parsesPlainParagraph() {
        val blocks = parseMarkdownBlocks("Hello world")
        assertEquals(1, blocks.size)
        assertTrue(blocks[0] is MdBlock.Paragraph)
        assertEquals("Hello world", (blocks[0] as MdBlock.Paragraph).text)
    }

    @Test
    fun parsesHeadingLevels() {
        val blocks = parseMarkdownBlocks("# H1\n## H2\n### H3")
        assertEquals(3, blocks.size)
        assertEquals(1, (blocks[0] as MdBlock.Heading).level)
        assertEquals("H1", (blocks[0] as MdBlock.Heading).text)
        assertEquals(2, (blocks[1] as MdBlock.Heading).level)
        assertEquals(3, (blocks[2] as MdBlock.Heading).level)
    }

    @Test
    fun parsesBulletList() {
        val blocks = parseMarkdownBlocks("- one\n- two\n* three")
        assertEquals(1, blocks.size)
        val list = blocks[0] as MdBlock.BulletList
        assertEquals(listOf("one", "two", "three"), list.items)
    }

    @Test
    fun parsesCodeBlockWithLanguage() {
        val blocks = parseMarkdownBlocks("```kotlin\nfun main() {}\n```")
        assertEquals(1, blocks.size)
        val code = blocks[0] as MdBlock.CodeBlock
        assertEquals("kotlin", code.language)
        assertEquals("fun main() {}", code.code)
    }

    @Test
    fun parsesMixedDocument() {
        val input = """
            # Title
            Intro paragraph.
            - first
            - second
            ```
            code
            ```
        """.trimIndent()
        val blocks = parseMarkdownBlocks(input)
        assertEquals(4, blocks.size)
        assertTrue(blocks[0] is MdBlock.Heading)
        assertTrue(blocks[1] is MdBlock.Paragraph)
        assertTrue(blocks[2] is MdBlock.BulletList)
        assertTrue(blocks[3] is MdBlock.CodeBlock)
    }

    @Test
    fun ignoresBlankLines() {
        val blocks = parseMarkdownBlocks("\n\nHello\n\n")
        assertEquals(1, blocks.size)
        assertEquals("Hello", (blocks[0] as MdBlock.Paragraph).text)
    }
}
