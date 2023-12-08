from typing import List
from .osml import html, DangerouslySetInnerHTML

"""
<!DOCTYPE html>
<html lang="en">
  <head>
    <title>Network</title>
    <script
      type="text/javascript"
      src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js"
    ></script>
    <style type="text/css">
      #mynetwork {
        width: 600px;
        height: 400px;
        border: 1px solid lightgray;
      }
    </style>
  </head>
  <body>
    <div id="mynetwork"></div>
    <script type="text/javascript">
      // create an array with nodes
      var nodes = new vis.DataSet([
        { id: 1, label: "Node 1" },
        { id: 2, label: "Node 2" },
        { id: 3, label: "Node 3" },
        { id: 4, label: "Node 4" },
        { id: 5, label: "Node 5" },
      ]);

      // create an array with edges
      var edges = new vis.DataSet([
        { from: 1, to: 3 },
        { from: 1, to: 2 },
        { from: 2, to: 4 },
        { from: 2, to: 5 },
        { from: 3, to: 3 },
      ]);

      // create a network
      var container = document.getElementById("mynetwork");
      var data = {
        nodes: nodes,
        edges: edges,
      };
      var options = {};
      var network = new vis.Network(container, data, options);
    </script>
  </body>
</html>

"""

def get_vis_network_cdn_script():
    return html.script(
        type="text/javascript",
        src="https://unpkg.com/vis-network/standalone/umd/vis-network.min.js",
    )

def get_example_graph_html():
    return html.div(
        html.div(id="mynetwork"),
        html.script(DangerouslySetInnerHTML("""
console.log("Running graph create example script");
// create an array with nodes
var nodes = new vis.DataSet([
{ id: 1, label: "Node 1" },
{ id: 2, label: "Node 2" },
{ id: 3, label: "Node 3" },
{ id: 4, label: "Node 4" },
{ id: 5, label: "Node 5" },
]);

// create an array with edges
var edges = new vis.DataSet([
{ from: 1, to: 3 },
{ from: 1, to: 2 },
{ from: 2, to: 4 },
{ from: 2, to: 5 },
{ from: 3, to: 3 },
]);

// create a network
var container = document.getElementById("mynetwork");
var data = {
nodes: nodes,
edges: edges,
};
var options = {};
var network = new vis.Network(container, data, options);
"""))
    )


def get_html_graph():
    return html.div(
        id="updateArea",
    )(
        get_vis_network_cdn_script(),
        get_example_graph_html(),
    )

if __name__ == "__main__":
    print(get_html_graph().render_html())

