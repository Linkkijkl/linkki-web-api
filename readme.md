# Linkki-Web Api

Dynamic content provider for [Linkki-web](https://github.com/linkkijkl/linkki-web).
Hosted at [api.linkkijkl.fi](https://api.linkkijkl.fi).

## Available endpoints
### [/events](https://api.linkkijkl.fi/events)
Returns all upcoming events. The events are fetched from Linkki's publicly available event calendar, and are cached for 10 minutes.

The endpoint returns a JSON object comforming to the following schema:
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://api.linkkijkl.fi/events",
  "title": "Events",
  "description": "List of upcoming events",
  "type": "array",
  "items": {
    "title": "Event",
    "description": "Information of an upcoming event",
    "type": "object",
    "properties": {
      "summary": {
        "type": "string",
        "title": "Event title"
      },
      "date": {
        "type": "string",
        "title": "Event date",
        "description": "Event start and end timestamps in human readable form"
      },
      "start_iso8601": {
        "type": "string",
        "title": "Event start timestamp",
        "description": "iso8601 formatted event start timestamp"
      },
      "end_iso8601": {
        "type": "string",
        "title": "Event end timestamp",
        "description": "iso8601 formatted event end timestamp"
      },
      "location": {
        "type": "object",
        "properties": {
          "string": {
            "type": "string",
            "title": "Event location"
          },
          "url": {
            "type": "string",
            "title": "Event location url"
          }
        }
      }
    }
  }
}
```