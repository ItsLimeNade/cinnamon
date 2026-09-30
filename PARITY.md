# Nightscout API parity

This table maps the Nightscout (15.0.x) endpoints to cinnamon. "v3 / v1" means cinnamon
uses API v3 when it's available and falls back to v1. Tests prefixed `e2e::` run against a
real Nightscout (see `tests/e2e.rs`).

## Collections

| Nightscout | cinnamon | Test |
|---|---|---|
| `GET /api/v3/{col}`, `GET /api/v1/{col}.json?find…&count` | `ns.<col>().list()…` (`.await`, `.stream()`, `.send_raw(fields)`) | `e2e::streams_paginate_across_equal_timestamps`, `e2e::v1_sees_old_entries_and_addresses_them_by_id` |
| `GET /api/v3/{col}/{id}`, v1 `find[_id]` / `find[identifier]` | `.get(id)` | `e2e::sgv_lifecycle_v3`, `e2e::missing_documents_are_not_found` |
| `POST /api/v3/{col}`, `POST /api/v1/{col}` | `.create(&doc)`, `.create_many(&docs)` | `e2e::treatments_v3_dedup_and_typed_view` |
| `PUT /api/v3/{col}/{id}`, `PUT /api/v1/{col}` | `.update(&doc)` (patches legacy docs through v3) | `e2e::legacy_treatment_is_updatable_through_v3` |
| `PATCH /api/v3/{col}/{id}` | `.patch(id, &json)` | `e2e::sgv_lifecycle_v3` |
| `DELETE /api/v3/{col}/{id}[?permanent=true]`, v1 by `_id` / identifier | `.delete(id)[.permanent()]` | `e2e::sgv_lifecycle_v3` |
| `DELETE /api/v1/{col}?find…` (server-side), v3 search + delete | `.delete_many(DeleteFilter)` | `e2e::delete_many_v1`, `e2e::delete_many_v3` |
| `GET /api/v3/{col}/history/{t}` | `.history(since, limit)` | `e2e::sgv_lifecycle_v3` |
| `GET /api/v1/profile/current` | `ns.profiles().current()` | `e2e::profiles_food_settings_activity` |
| `GET /api/v1/food/quickpicks` | `ns.food().quickpicks()` | `e2e::profiles_food_settings_activity` |
| `/api/v3/settings` | `ns.settings()` (v3 only) | `e2e::profiles_food_settings_activity` |
| `/api/v1/activity` | `ns.activity()` (v1 only) | `e2e::profiles_food_settings_activity` |

Collections covered: entries (`sgv`, `mbg`, `cal`, all), treatments, devicestatus, profile,
food, settings and activity.

## Entries helpers (API v1)

| Nightscout | cinnamon |
|---|---|
| `GET /api/v1/entries/current` | `ns.entries().current()` |
| `GET /api/v1/times/{prefix}/{regex}` | `ns.entries().times(prefix, regex, count)` |
| `GET /api/v1/slice/{storage}/{field}/{type}/{prefix}/{regex}` | `ns.entries().slice(..)` |
| `GET /api/v1/count/entries/where` | `ns.entries().count(&filter)` |
| `GET /api/v1/echo/{storage}` | `ns.entries().echo(storage, &filter)` |

## Server, auth and computed state

| Nightscout | cinnamon |
|---|---|
| `GET /api/versions` | `ns.server().versions()` |
| `GET /api/v1/status.json` | `ns.server().status()` |
| `GET /api/v3/version` / `status` / `lastModified` | `ns.server().v3_version()` / `v3_status()` / `last_modified()` |
| `GET /api/v1/verifyauth` | `ns.server().verify_auth()` |
| `GET /api/v1/adminnotifies` | `ns.server().admin_notifies()` |
| `GET /api/v2/authorization/request/{token}` | automatic (JWT exchange and refresh); `ns.session()` |
| `/api/v2/authorization/subjects`, `roles`, `permissions`, `debug/check/{perm}` | `ns.admin().…` |
| `GET /api/v2/properties[/names]` | `ns.properties().only([...])` |
| `GET /api/v2/ddata/at/{time}` | `ns.ddata(Some(time))` |
| `GET /api/v2/summary?hours=` | `ns.summary(hours)` |
| `GET /api/v1/notifications/ack` | `ns.notifications().ack(level, group, silence)` |
| `POST /api/v2/notifications/loop` | `ns.notifications().push_to_loop(&LoopNotification)` |

## Realtime (feature `realtime`)

| Nightscout | cinnamon | Test |
|---|---|---|
| socket.io `/storage` (`create`, `update`, `delete`) | `ns.realtime().storage([...])` | `e2e::realtime::storage_channel_sees_v3_writes` |
| socket.io `/alarm` (`alarm`, `urgent_alarm`, `announcement`, `notification`, `clear_alarm`, `ack`) | `ns.realtime().alarms()`, `Subscription::ack` | `e2e::realtime::alarm_channel_subscribes_and_acks` |
| socket.io root `authorize` + `dataUpdate` | `ns.realtime().updates()`, `ns.realtime().glucose()` | `e2e::realtime::legacy_updates_and_glucose_follow_v1_uploads` |

## Incremental sync

`/lastModified`, history and a date-window search for legacy uploads → `ns.sync(cursor)`.
Covered by `e2e::sync_sees_creates_updates_deletes_and_legacy_uploads`.

## Not covered

- `/pebble`, `/api/v1/alexa`, `/api/v1/googlehome`: voice and watch face integrations, not
  client APIs.
- Legacy socket `dbAdd` / `dbUpdate` / `dbRemove`: use the REST API instead, which
  normalizes dates and is covered by permissions.
- Client-side analytics (IOB/COB/AR2 and reports): deferred.
