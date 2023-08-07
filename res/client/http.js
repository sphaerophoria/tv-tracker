export async function request_episodes(show_id) {
  const params = new URLSearchParams({ show_id: show_id });
  const request = new Request("/episodes?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function request_shows() {
  const request = new Request("/shows", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function request_shows_by_watch_status() {
  const request = new Request("/shows_by_watch_status", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function request_watch_status(show_id) {
  const params = new URLSearchParams({ show_id: show_id });
  const request = new Request("/watch_status?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function request_set_watch_status(episode_id, watch_date) {
  const request = new Request("/watch_status", {
    method: "PUT",
    body: JSON.stringify({
      episode_id: episode_id,
      watch_date: watch_date,
    }),
  });
  return await fetch(request);
}
