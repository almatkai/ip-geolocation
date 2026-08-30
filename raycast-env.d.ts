/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** Language - Language of location information display. */
  "language": "en" | "de" | "es" | "pt-BR" | "fr" | "ja" | "zh-CN" | "ru",
  /** Coordinates Format - Coordinate format of location information display. */
  "coordinatesFormat": "latLon" | "lonLat"
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `query-ip-geolocation` command */
  export type QueryIpGeolocation = ExtensionPreferences & {}
  /** Preferences accessible in the `my-ip-geolocation` command */
  export type MyIpGeolocation = ExtensionPreferences & {
  /** undefined - Query and display IPv6 address. Can cause information loading speed to drop. */
  "showIPv6": boolean
}
  /** Preferences accessible in the `copy-ip` command */
  export type CopyIp = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `query-ip-geolocation` command */
  export type QueryIpGeolocation = {
  /** IP Address or Domain */
  "ipAddress": string
}
  /** Arguments passed to the `my-ip-geolocation` command */
  export type MyIpGeolocation = {}
  /** Arguments passed to the `copy-ip` command */
  export type CopyIp = {
  /** Local */
  "ipType": "Public",
  /** IPv4 */
  "ipVersion": "IPv6"
}
}

