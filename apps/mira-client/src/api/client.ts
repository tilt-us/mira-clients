import { client } from "./generated/client.gen";
import { API_BASE_URL, CHAMPION_API_BASE_URL } from "./config";
import { apiFetch, getClientDeviceType } from "./http";
import { getValidAccessToken, getValidDesktopApiToken } from "../auth/keycloak";

export type LobbyRole = "TOP" | "JUNGLE" | "MID" | "ADC" | "SUPPORT";

export type LobbyRoleMember = {
  avatarUrl?: string;
  displayName?: string;
  primaryRole?: LobbyRole;
  publicId?: number;
  secondaryRole?: LobbyRole;
};

export type LobbyRolesSnapshot = {
  lobbyId?: string;
  members?: LobbyRoleMember[];
};

type UpdateLobbyMemberRolesOptions = {
  baseUrl?: string;
  body: {
    primaryRole?: LobbyRole;
    secondaryRole?: LobbyRole | null;
  };
  fallbackBaseUrls?: string[];
  path: {
    lobbyId: string;
  };
};

type GetLobbyRolesOptions = {
  baseUrl?: string;
  fallbackBaseUrls?: string[];
  path: {
    lobbyId: string;
  };
};

type DeleteChatRoomOptions = {
  baseUrl?: string;
  path: {
    roomId: string;
  };
};

type GetChampionCatalogOptions = {
  query?: {
    owned?: boolean;
    userId?: number;
    weekly?: boolean;
  };
};

type SetOwnedChampionOptions = {
  body: {
    champion: number | string;
    userId: number;
  };
};

client.setConfig({
  baseUrl: API_BASE_URL,
  fetch: apiFetch,
});

client.interceptors.request.use(async (request) => {
  const deviceType = getClientDeviceType();
  const accessToken =
    deviceType === "Desktop" ? await getValidDesktopApiToken() : await getValidAccessToken();

  request.headers.set("X-Device-Type", deviceType);

  if (accessToken) {
    request.headers.set("authorization", `Bearer ${accessToken}`);
  } else {
    request.headers.delete("authorization");
  }

  return request;
});

export function setApiAccessToken(accessToken?: string) {
  client.setConfig({
    baseUrl: API_BASE_URL,
    fetch: apiFetch,
    headers: {
      Authorization: accessToken ? `Bearer ${accessToken}` : null,
    },
  });
}

export function updateLobbyMemberRoles(options: UpdateLobbyMemberRolesOptions) {
  const { fallbackBaseUrls = [], ...requestOptions } = options;

  async function putRoles(baseUrl?: string) {
    return client.put<{ 200: LobbyRolesSnapshot }, unknown, false>({
      url: "/api/lobbies/{lobbyId}/members/me/roles",
      ...requestOptions,
      baseUrl,
      headers: {
        "Content-Type": "application/json",
      },
    });
  }

  return putRoles(options.baseUrl).then(async (result) => {
    if (result.response?.status !== 404) {
      return result;
    }

    for (const fallbackBaseUrl of fallbackBaseUrls) {
      const fallbackResult = await putRoles(fallbackBaseUrl);

      if (fallbackResult.response?.status !== 404) {
        return fallbackResult;
      }
    }

    return result;
  });
}

export function getLobbyRoles(options: GetLobbyRolesOptions) {
  const { fallbackBaseUrls = [], ...requestOptions } = options;

  async function getRoles(baseUrl?: string) {
    return client.get<{ 200: LobbyRolesSnapshot }, unknown, false>({
      url: "/api/lobbies/{lobbyId}/roles",
      ...requestOptions,
      baseUrl,
    });
  }

  return getRoles(options.baseUrl).then(async (result) => {
    if (result.response?.status !== 404) {
      return result;
    }

    for (const fallbackBaseUrl of fallbackBaseUrls) {
      const fallbackResult = await getRoles(fallbackBaseUrl);

      if (fallbackResult.response?.status !== 404) {
        return fallbackResult;
      }
    }

    return result;
  });
}

export function deleteChatRoom(options: DeleteChatRoomOptions) {
  return client.delete<{ 204: void; 409: unknown }, unknown, false>({
    url: "/api/chats/rooms/{roomId}",
    ...options,
  }).then((result) => {
    if (result.response?.status !== 404) {
      return result;
    }

    return client.delete<{ 204: void; 409: unknown }, unknown, false>({
      url: "/api/chats/{roomId}",
      ...options,
    });
  });
}

export function getChampionCatalog(options?: GetChampionCatalogOptions) {
  return client.get<{ 200: unknown }, unknown, false>({
    url: "/api/champions",
    baseUrl: CHAMPION_API_BASE_URL,
    query: options?.query,
  });
}

export function setOwnedChampion(options: SetOwnedChampionOptions) {
  return client.post<{ 200: unknown; 201: unknown }, unknown, false>({
    url: "/api/champions/owned",
    baseUrl: CHAMPION_API_BASE_URL,
    ...options,
    headers: {
      "Content-Type": "application/json",
    },
  });
}

export { client };
export type {
  ApiChampionHoverRequest,
  ApiMatchChampionHoversResponse,
  ApiMatchResponse,
} from "./generated";
export * from "./generated";
