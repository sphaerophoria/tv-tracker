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

export async function request_paused_shows() {
  const request = new Request("/pause_status", {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}

export async function request_set_pause_status(show_id, pause_status) {
  const request = new Request("/pause_status", {
    method: "PUT",
    body: JSON.stringify({
      show_id: show_id,
      paused: pause_status,
    }),
  });
  return await fetch(request);
}

export async function request_remove_show(show_id) {
  const request = new Request("/remove_show", {
    method: "PUT",
    body: JSON.stringify({
      show_id: show_id,
    }),
  });
  return await fetch(request);
}

export async function request_episodes_aired_between(start_date, end_date) {
  const params = new URLSearchParams({
    start_date: start_date,
    end_date: end_date,
  });
  const request = new Request("/episodes_aired_between?" + params.toString(), {
    method: "GET",
  });
  const response = await fetch(request);
  return await response.json();
}
