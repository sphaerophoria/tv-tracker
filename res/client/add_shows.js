"use strict";

function handle_search(element) {
  if (event.key === "Enter") {
    const request = new Request("/search?" + element.value, {
      method: "GET",
    });
    fetch(request);
  }
}

function get_search_button() {
  return document.getElementById("show-search-button");
}

function get_search_box() {
  return document.getElementById("show-search");
}

function handle_search_keypress(event) {
  if (event.key === "Enter") {
    event.preventDefault();
    get_search_button().click();
  }
}

function render_show(item) {
  var rendered = '<div class="show-card">';

  if (item.url !== null) {
    rendered += "<a href=" + item.url + ">";
  }

  if (item.image !== null) {
    rendered += '<img class="card-image" src=' + item.image + ">";
  } else {
    rendered += '<div class="card-placeholder-image"></div>';
  }

  rendered += item.name;

  if (item.year !== null) {
    rendered += " (" + item.year + ")";
  }

  if (item.url !== null) {
    rendered += "</a>";
  }

  rendered += "</div>";

  return rendered;
}

async function execute_search() {
  const params = new URLSearchParams({ query: get_search_box().value });
  const request = new Request("/search?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  const response_body = await response.json();

  const search_results = document.getElementById("search-results");

  var new_html = "";

  for (const i in response_body) {
    const item = response_body[i];
    new_html += render_show(item);
  }

  search_results.innerHTML = new_html;
}

function setup_search_handlers() {
  const search_box = get_search_box();
  search_box.addEventListener("keypress", handle_search_keypress);
  get_search_button().onclick = execute_search;
}

async function init() {
  setup_search_handlers();
}

init();
