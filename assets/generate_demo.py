from PIL import Image, ImageDraw, ImageFont
import textwrap

# Canvas
W, H = 900, 700
bg = "#1e1e1e"
img = Image.new("RGB", (W, H), bg)
draw = ImageDraw.Draw(img)

# Fonts
try:
    font = ImageFont.truetype("Consolas", 16)
    font_bold = ImageFont.truetype("Consolas", 16)
except:
    font = ImageFont.load_default()
    font_bold = ImageFont.load_default()

# Layout
margin = 20
line_h = 24
y = margin + 40  # header space

def draw_line(text, color, indent=0, bold=False):
    global y
    f = font_bold if bold else font
    draw.text((margin + indent, y), text, fill=color, font=f)
    y += line_h

def draw_block(label, lines, label_color, text_color):
    global y
    draw.text((margin, y), label, fill=label_color, font=font_bold)
    y += line_h
    for line in lines:
        draw.text((margin + 16, y), line, fill=text_color, font=font)
        y += line_h
    y += 4

# Header bar
draw.rectangle([0, 0, W, 36], fill="#2d2d2d")
draw.text((margin, 8), "Clarity TUI  —  Kimi-k2-07132k  —  Direct", fill="#cccccc", font=font)

# Messages
draw_line("[System] 欢迎使用 Clarity! 输入 /help 查看可用命令。", "#888888")
y += 8

draw_line("[User] 请读取 README.md 的前5行", "#4fc1ff", bold=True)
y += 8

draw_line("[Assistant] 我将使用 MCP 工具来读取 README.md 的内容。", "#d4d4d4")
y += 8

draw_line("🔧  ToolCall   filesystem_read_file", "#ffcc00", bold=True)
draw_line('      { "path": "README.md" }', "#ffcc00")
y += 8

draw_line("✅  ToolResult  filesystem_read_file  —  success", "#89d185")
readme_lines = [
    "# Project Clarity",
    "",
    "> Local-first AI Agent runtime in Rust.",
    "",
    "---",
]
for line in readme_lines:
    draw.text((margin + 16, y), line, fill="#b5cea8", font=font)
    y += line_h
y += 8

draw_line("[Assistant] 已为您读取 README.md 的前5行（见上文工具结果）。", "#d4d4d4")
y += 8

# Footer / input box
draw.rectangle([0, H - 40, W, H], fill="#252526")
draw.text((margin, H - 28), "> 请读取 README.md 的前5行", fill="#cccccc", font=font)

# Metrics HUD on right
draw.rectangle([W - 180, 50, W - 20, 180], fill="#252526", outline="#3c3c3c")
metrics = [
    "Metrics",
    "",
    "Latency:  1.2s",
    "TTFT:     0.4s",
    "Tokens:   142",
    "Step:     1/10",
]
my = 60
for m in metrics:
    draw.text((W - 170, my), m, fill="#cccccc", font=font)
    my += line_h

# Save
img.save("C:\\Users\\22414\\Desktop\\clarity\\assets\\tui_demo.png")
print("Saved tui_demo.png")
