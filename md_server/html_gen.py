from .osml import Component, html, DangerouslySetInnerHTML



def get_script():
    return html.script(
        type="module",
    )(DangerouslySetInnerHTML(
"""
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@10/dist/mermaid.esm.min.mjs';

self.currentTheme = 'forest';  // default theme

// Function to initialize or re-render Mermaid diagrams with a specific theme
function renderMermaid(theme) {
    self.currentTheme = theme;
    mermaid.initialize({ 
        startOnLoad: true,
        theme: theme,
    });
    // Re-render Mermaid diagrams
    mermaid.init(undefined, document.querySelectorAll('.mermaid'));
}

// Initial render with default theme
console.log('Rendering Mermaid diagrams with default theme');
renderMermaid('forest');

function changePrismTheme(cssFile) {
    var prismLink = document.getElementById('prism-theme-link');
    if (!prismLink) {
        prismLink = document.createElement('link');
        prismLink.rel = 'stylesheet';
        prismLink.id = 'prism-theme-link';
        document.head.appendChild(prismLink);
    }
    prismLink.href = "/static/prism/" + cssFile;  // Update the path as needed
    console.log('Changed Prism theme to: ' + cssFile);
}




document.body.addEventListener('htmx:sseMessage', function(event) {
    const updateArea = document.getElementById('updateArea');
    updateArea.innerHTML = updateArea.innerHTML.replace(/\[NEWLINE\]/g, "\\n");
    // Re-render Mermaid diagrams after updating the content
    console.log('Re-rendering Mermaid diagrams');
    renderMermaid(self.currentTheme);
    Prism.highlightAll();
});

document.body.addEventListener('htmx:afterSwap', function(event) {
    const updateArea = document.getElementById('updateArea');
    updateArea.innerHTML = updateArea.innerHTML.replace(/\[NEWLINE\]/g, "\\n");
    // Re-render Mermaid diagrams after updating the content
    console.log('Re-rendering Mermaid diagrams');
    renderMermaid(self.currentTheme);
    Prism.highlightAll();
});

function changeCSS(cssFile) {
    document.getElementById('theme-link').href = "/static/" + cssFile;
}

document.getElementById('css-selector').addEventListener('change', function() {
    changeCSS(this.value);
    // Determine Mermaid theme based on the selected CSS file
    var selectedCSS = this.value;
    var mermaidTheme = selectedCSS.includes('dark') ? 'dark' : 'forest';
    console.log('Changing Mermaid theme to: ' + mermaidTheme);
    renderMermaid(mermaidTheme);

    var prismTheme = selectedCSS.includes('dark') ? 'prism-okaidia.css' : 'prism.css'; // Example theme names
    changePrismTheme(prismTheme);
});

"""
      ))



def markdown_update_div():
    return html.div(
        id="updateArea", 
        class_="markdown-body", 
        hx_ext="sse", 
        sse_connect="/sse", 
        sse_swap="message",
        style={
            "width": "92%",
            "margin": "auto",
        },
        ondragover="event.preventDefault()",
        ondrop="handleDrop(event)"  
    )(
        html.h3("Listening to updates gen html"),
        html.h3("Update and save a new file to get started..."),
        html.pre(class_="mermaid")(
            "graph TD",
            "A[Edit a file] -->|SSE| B",
            "B[Update rendered HTML]"
        ),
        html.code(
            class_="language-python",
        )(
"""
import os

def sample_function():
    print("Hello world!")

"""),
            
        html.script(DangerouslySetInnerHTML(
"""
function handleDrop(event) {
    console.log('File(s) dropped');
    console.log(event);
    event.preventDefault();
    console.log(event.dataTransfer.files);
    if (event.dataTransfer && event.dataTransfer.files.length) {
        const file = event.dataTransfer.files[0];
        console.log('File dropped:', file.name);
        console.log('File type:', file.type);
        if (file.type.startsWith('image/')) {
            uploadImage(file);
        } else {
            console.error('File is not an image');
        }
    }
}

function uploadImage(file) {
    console.log('Uploading image...');
    const formData = new FormData();
    formData.append('file', file);

    fetch('/upload_image', {
        method: 'POST',
        body: formData
    }).then(response => {
        if (response.ok) {
            console.log('Image successfully uploaded');
            // Optional: Update UI to show uploaded image or success message
        } else {
            console.error('Error uploading image');
        }
    }).catch(error => {
        console.error('Error:', error);
    });
}

"""
        ))
    )



def get_navbar(css_options):
    html_options = []
    for css_option in css_options:
        html_options.append(html.option(css_option))

    dropdown_style = {
        "border-radius": "10px",
        "padding": "5px",
        "width": "200",
        "position": "relative",
        "background-color": "rgba(0, 0, 0, 0.0)",
    }

    select_style = {
        "width": "200",
        "appearance": "none",
        "-webkit-appearance": "none",
        "padding": "0.375em 3em 0.375em 1em",
        "border-radius": "10px",
        "border": "1px solid rgba(100, 100, 100, 0.8)",
        "cursor": "pointer",
        "background-color": "rgba(0, 0, 0, 0.0)",
    }

    return html.div(
        class_="navbar", 
        style={
            "display": "flex",
            "justify-content": "space-around",
            "align-items": "center",
        }
        )(
        html.div(
            class_="dropdown",
            style=dropdown_style,
        )(
            html.select(class_="dropdown-content", id="css-selector", hx_get="/last-sent", hx_target="#updateArea", style=select_style)(
                *html_options
            )
        ),
        html.div(
            html.button(
                hx_get="/get_graph",
                hx_target="#updateArea",
                style={
                    "border-radius": "10px",
                    "padding": "5px",
                    "width": "200px",
                    "background-color": "rgba(0, 0, 0, 0.0)",
                    "border": "1px solid rgba(100, 100, 100, 0.8)",
                    "cursor": "pointer",
                }

            )(
                "Graph view",
            )
        ),

        html.script(DangerouslySetInnerHTML("""

        """
        ))

    )


def emmpty_div():
    return html.div(
        style={
            "height": "1000px",
        }
    )()

def get_body(css_options):
    return html.body(class_="markdown-body")(
        get_navbar(css_options),
        markdown_update_div(),
        get_script(),
        emmpty_div(),
    )



def get_html(css_options):
    html_comp = html.doctype(
        html.html(
            html.head(
                html.title("Last Modifed File Tracker"),
                html.meta(charset="utf-8"),
                html.link(rel="stylesheet", href="https://cdnjs.cloudflare.com/ajax/libs/prism/1.24.1/themes/prism.min.css"),
                html.link(rel="stylesheet", href=f"/static/{css_options[0]}", id="theme-link"),
                html.link(rel="preconnect", href="https://fonts.googleapis.com"),
                html.link(rel="preconnect", href="https://fonts.gstatic.com", crossorigin=""),
                html.link(href="https://fonts.googleapis.com/css2?family=Roboto&display=swap", rel="stylesheet"),
                html.script(src="https://unpkg.com/htmx.org"),
                html.script(src="https://unpkg.com/htmx.org/dist/ext/sse.js"),
                html.script(src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.24.1/prism.min.js"),
                html.script(src="https://cdnjs.cloudflare.com/ajax/libs/prism/1.24.1/components/prism-python.min.js"),
            ),
            get_body(css_options),
        ),
    )
    return html_comp.render_html()









