import { MessageCircle, Send, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  deleteChatRoom,
  listMessages,
  listRooms,
  markRead,
  sendLobbyMessage,
  sendPrivate,
  sendTeamMessage,
  type ChatMessageResponse,
  type ChatRoomResponse,
} from "../api/client";
import { CHAT_API_BASE_URL } from "../api/config";
import type { AppLocale } from "../i18n";
import type { Translate } from "../types/ui";
import { getProfileInitials } from "../utils/profile";
import type { ChatPosition } from "../settings";

type ChatDockProps = {
  autoRooms?: ChatRoom[];
  chatPosition: ChatPosition;
  currentUserPublicId?: number;
  locale: AppLocale;
  placement?: "default" | "fullscreen";
  t: Translate;
};

export type ChatRoom = {
  avatarUrl?: string;
  contextId?: string;
  id: string;
  locked?: boolean;
  lastReadAt?: string;
  participants?: ChatParticipant[];
  name: string;
  roomId?: string;
  subtitle?: string;
  targetPublicId?: number;
  team?: "Dark" | "Light";
  type?: "direct" | "lobby" | "team";
};

export type ChatParticipant = {
  avatarUrl?: string;
  name: string;
  publicId: number;
};

type ChatContact = ChatRoom & {
  lastActivityKey?: string;
  messages?: ChatMessageResponse[];
  messagesLoaded?: boolean;
  messagesLoading?: boolean;
  sendError?: boolean;
  sending?: boolean;
  unreadCount?: number;
};

type ChatSendResult = {
  data?: ChatMessageResponse;
  error?: unknown;
};

type ChatRequestEvent = CustomEvent<{
  avatarUrl?: string;
  friendId?: string;
  name?: string;
  publicId?: number;
}>;

type ChatFriendsUpdatedEvent = CustomEvent<{
  friends?: Array<{
    avatarUrl?: string;
    name?: string;
    publicId?: number;
  }>;
}>;

const emptyChatRooms: ChatRoom[] = [];
const chatRefreshMs = 5_000;
const chatMessageLimit = 50;

function isChatRequestEvent(event: Event): event is ChatRequestEvent {
  return event.type === "mira:chat-request";
}

function isChatFriendsUpdatedEvent(
  event: Event,
): event is ChatFriendsUpdatedEvent {
  return event.type === "mira:friends-updated";
}

function isAutoChatRoomId(contactId: string) {
  return contactId.startsWith("lobby:") || contactId.startsWith("team:");
}

function areChatContactsEqual(left: ChatContact[], right: ChatContact[]) {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((leftContact, index) => {
    const rightContact = right[index];

    return (
      leftContact.avatarUrl === rightContact.avatarUrl &&
      leftContact.id === rightContact.id &&
      leftContact.locked === rightContact.locked &&
      areChatParticipantsEqual(leftContact.participants, rightContact.participants) &&
      leftContact.contextId === rightContact.contextId &&
      leftContact.lastReadAt === rightContact.lastReadAt &&
      leftContact.roomId === rightContact.roomId &&
      leftContact.name === rightContact.name &&
      leftContact.subtitle === rightContact.subtitle &&
      leftContact.targetPublicId === rightContact.targetPublicId &&
      leftContact.team === rightContact.team &&
      leftContact.type === rightContact.type &&
      leftContact.unreadCount === rightContact.unreadCount
    );
  });
}

function areChatParticipantsEqual(
  left: ChatParticipant[] | undefined,
  right: ChatParticipant[] | undefined,
) {
  if ((left?.length ?? 0) !== (right?.length ?? 0)) {
    return false;
  }

  return (left ?? []).every((leftParticipant, index) => {
    const rightParticipant = right?.[index];

    return (
      leftParticipant.avatarUrl === rightParticipant?.avatarUrl &&
      leftParticipant.name === rightParticipant?.name &&
      leftParticipant.publicId === rightParticipant?.publicId
    );
  });
}

function getPrivateContactId(publicId: number) {
  return `private:${publicId}`;
}

function toPublicId(value: unknown) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }

  if (typeof value === "string" && value.trim()) {
    const parsedValue = Number(value);

    return Number.isFinite(parsedValue) ? parsedValue : undefined;
  }

  return undefined;
}

function getRoomType(room: ChatRoomResponse) {
  return room.type?.toUpperCase();
}

function getRoomId(room: ChatRoomResponse | undefined) {
  if (!room) {
    return undefined;
  }

  const runtimeRoom = room as ChatRoomResponse & { id?: unknown };

  return (
    (typeof room.roomId === "string" && room.roomId) ||
    (typeof runtimeRoom.id === "string" && runtimeRoom.id) ||
    undefined
  );
}

function getMessageId(message: ChatMessageResponse | undefined) {
  if (!message) {
    return undefined;
  }

  const runtimeMessage = message as ChatMessageResponse & { id?: unknown };

  return (
    (typeof message.messageId === "string" && message.messageId) ||
    (typeof runtimeMessage.id === "string" && runtimeMessage.id) ||
    undefined
  );
}

function getMessageRoomId(message: ChatMessageResponse | undefined) {
  if (!message) {
    return undefined;
  }

  const runtimeMessage = message as ChatMessageResponse & { chatRoomId?: unknown };

  return (
    (typeof message.roomId === "string" && message.roomId) ||
    (typeof runtimeMessage.chatRoomId === "string" && runtimeMessage.chatRoomId) ||
    undefined
  );
}

function normalizeChatMessage(
  message: ChatMessageResponse | undefined,
): ChatMessageResponse | undefined {
  if (!message) {
    return undefined;
  }

  return {
    ...message,
    messageId: getMessageId(message) ?? message.messageId,
    roomId: getMessageRoomId(message) ?? message.roomId,
  };
}

function normalizeChatMessages(messages: ChatMessageResponse[]) {
  return messages
    .map((message) => normalizeChatMessage(message))
    .filter((message): message is ChatMessageResponse => Boolean(message));
}

function getRoomParticipantPublicIds(room: ChatRoomResponse) {
  return (
    room.participantPublicIds
      ?.map((publicId) => toPublicId(publicId))
      .filter((publicId): publicId is number => typeof publicId === "number") ?? []
  );
}

function getChatRooms(data: unknown): ChatRoomResponse[] {
  if (Array.isArray(data)) {
    return data as ChatRoomResponse[];
  }

  if (data && typeof data === "object") {
    const rooms = (data as { rooms?: unknown }).rooms;

    if (Array.isArray(rooms)) {
      return rooms as ChatRoomResponse[];
    }
  }

  return [];
}

function getChatMessages(data: unknown): ChatMessageResponse[] {
  if (Array.isArray(data)) {
    return normalizeChatMessages(data as ChatMessageResponse[]);
  }

  if (data && typeof data === "object") {
    const messages = (data as { messages?: unknown }).messages;

    if (Array.isArray(messages)) {
      return normalizeChatMessages(messages as ChatMessageResponse[]);
    }
  }

  return [];
}

function findPrivateRoom(
  rooms: ChatRoomResponse[] | undefined,
  targetPublicId: number,
) {
  if (!Array.isArray(rooms)) {
    return undefined;
  }

  return rooms?.find((room) => {
    return (
      getRoomType(room) === "PRIVATE" &&
      getRoomParticipantPublicIds(room).includes(targetPublicId)
    );
  });
}

function getPrivateRoomPeerPublicId(
  room: ChatRoomResponse,
  currentUserPublicId: number | undefined,
) {
  if (getRoomType(room) !== "PRIVATE" || typeof currentUserPublicId !== "number") {
    return undefined;
  }

  return getRoomParticipantPublicIds(room).find(
    (participantPublicId) => participantPublicId !== currentUserPublicId,
  );
}

function getFriendContactInfo(
  friendsByPublicId: Map<number, { avatarUrl?: string; name?: string }>,
  publicId: number,
  fallbackName: string,
) {
  const friend = friendsByPublicId.get(publicId);

  return {
    avatarUrl: friend?.avatarUrl,
    name: friend?.name ?? fallbackName,
  };
}

function chatContextContains(contextId: string | undefined, expectedContextId: string) {
  const normalizedContextId = contextId?.toUpperCase();
  const normalizedExpectedContextId = expectedContextId.toUpperCase();

  return (
    normalizedContextId === normalizedExpectedContextId ||
    Boolean(
      normalizedContextId?.startsWith(`${normalizedExpectedContextId}:`) ||
        normalizedContextId?.endsWith(`:${normalizedExpectedContextId}`) ||
        normalizedContextId?.includes(`:${normalizedExpectedContextId}:`),
    )
  );
}

function findChatRoomForContact(
  rooms: ChatRoomResponse[] | undefined,
  contact: ChatContact,
  currentUserPublicId: number | undefined,
) {
  if (contact.type === "direct" && typeof contact.targetPublicId === "number") {
    return findPrivateRoom(rooms, contact.targetPublicId);
  }

  if (contact.type === "lobby" && contact.contextId) {
    const exactRoom = rooms?.find((room) => {
      return (
        getRoomType(room) === "LOBBY" &&
        chatContextContains(room.contextId, contact.contextId as string)
      );
    });

    if (exactRoom) {
      return exactRoom;
    }

    return rooms?.find((room) => {
      const userCanAccessRoom =
        typeof currentUserPublicId !== "number" ||
        getRoomParticipantPublicIds(room).includes(currentUserPublicId);

      return getRoomType(room) === "LOBBY" && userCanAccessRoom;
    });
  }

  if (contact.type === "team" && contact.contextId) {
    const contactContextId = contact.contextId;
    const teamPath = getChatTeamPath(contact.team);
    const expectedContextIds = new Set(
      [
        contactContextId,
        teamPath ? `${contactContextId}:${teamPath}` : undefined,
        contact.team ? `${contactContextId}:${contact.team}` : undefined,
      ]
        .filter((contextId): contextId is string => Boolean(contextId))
        .map((contextId) => contextId.toUpperCase()),
    );

    return rooms?.find((room) => {
      if (getRoomType(room) !== "TEAM") {
        return false;
      }

      const userCanAccessRoom =
        typeof currentUserPublicId !== "number" ||
        getRoomParticipantPublicIds(room).includes(currentUserPublicId);

      const roomContextId = room.contextId?.toUpperCase();

      return (
        userCanAccessRoom &&
        (!roomContextId ||
          expectedContextIds.has(roomContextId) ||
          chatContextContains(room.contextId, contactContextId))
      );
    });
  }

  return undefined;
}

function getMessageSenderInfo(
  contact: ChatContact | undefined,
  senderPublicId: number | undefined,
  friendsByPublicId: Map<number, { avatarUrl?: string; name?: string }>,
  t: Translate,
) {
  if (typeof senderPublicId !== "number") {
    return {
      avatarUrl: undefined,
      name: t("chat-unknown-contact"),
    };
  }

  const participant = contact?.participants?.find((currentParticipant) => {
    return currentParticipant.publicId === senderPublicId;
  });

  if (participant) {
    return participant;
  }

  const friend = friendsByPublicId.get(senderPublicId);

  if (friend) {
    return {
      avatarUrl: friend.avatarUrl,
      name: friend.name ?? `#${senderPublicId}`,
      publicId: senderPublicId,
    };
  }

  return {
    name: `#${senderPublicId}`,
    publicId: senderPublicId,
  };
}

function isIncomingRoomMessage(
  message: ChatMessageResponse | undefined,
  currentUserPublicId: number,
): message is ChatMessageResponse & { content: string; senderPublicId: number } {
  return (
    hasMessageContent(message) &&
    typeof message.senderPublicId === "number" &&
    message.senderPublicId !== currentUserPublicId
  );
}

function isAfterLastRead(message: ChatMessageResponse, lastReadAt: string | undefined) {
  if (!lastReadAt) {
    return true;
  }

  const messageTime = message.createdAt ? Date.parse(message.createdAt) : Number.NaN;
  const lastReadTime = Date.parse(lastReadAt);

  return (
    !Number.isFinite(messageTime) ||
    !Number.isFinite(lastReadTime) ||
    messageTime > lastReadTime
  );
}

function getRoomUnreadCount(
  room: ChatRoomResponse,
  currentUserPublicId: number,
) {
  if ((room.unreadCount ?? 0) > 0) {
    return room.unreadCount ?? 0;
  }

  const lastMessage = room.lastMessage;

  if (
    isIncomingRoomMessage(lastMessage, currentUserPublicId) &&
    isAfterLastRead(lastMessage, room.lastReadAt)
  ) {
    return 1;
  }

  return room.unreadCount ?? 0;
}

function getMessageActivityKey(message: ChatMessageResponse | undefined) {
  if (!message) {
    return undefined;
  }

  return (
    getMessageId(message) ??
    `${message.senderPublicId ?? "unknown"}:${message.createdAt ?? ""}:${
      message.content ?? ""
    }`
  );
}

function getRoomActivityKey(room: ChatRoomResponse | undefined) {
  if (!room) {
    return undefined;
  }

  return (
    getMessageActivityKey(room.lastMessage) ??
    room.updatedAt ??
    room.lastReadAt ??
    getRoomId(room)
  );
}

function hasPrivateRoomActivity(
  room: ChatRoomResponse,
  currentUserPublicId: number,
) {
  return (
    getRoomUnreadCount(room, currentUserPublicId) > 0 ||
    isIncomingRoomMessage(room.lastMessage, currentUserPublicId)
  );
}

function canSendToContact(contact: ChatContact | undefined) {
  if (!contact || contact.sending) {
    return false;
  }

  if (contact.type === "direct") {
    return typeof contact.targetPublicId === "number";
  }

  if (contact.type === "lobby") {
    return Boolean(contact.contextId);
  }

  if (contact.type === "team") {
    return Boolean(contact.contextId && contact.team);
  }

  return false;
}

function getChatTeamPath(team: ChatContact["team"]) {
  return team?.toUpperCase();
}

function getChatContactResetKey(contact: ChatContact | ChatRoom | undefined) {
  if (!contact) {
    return "";
  }

  return [
    contact.type ?? "",
    contact.contextId ?? "",
    contact.team ?? "",
    contact.participants?.map((participant) => participant.publicId).join(",") ?? "",
  ].join(":");
}

function getChatTeamPathCandidates(team: ChatContact["team"]) {
  const numericTeam = team === "Dark" ? "0" : team === "Light" ? "1" : undefined;

  return Array.from(
    new Set(
      [team?.toUpperCase(), team, team?.toLowerCase(), numericTeam].filter(
        (teamPath): teamPath is string => Boolean(teamPath),
      ),
    ),
  );
}

function isLastKnownGroupParticipant(
  contact: ChatContact,
  currentUserPublicId: number | undefined,
) {
  return (
    typeof currentUserPublicId === "number" &&
    (contact.type === "lobby" || contact.type === "team") &&
    (contact.participants?.length ?? 0) > 0 &&
    contact.participants?.every(
      (participant) => participant.publicId === currentUserPublicId,
    )
  );
}

function sortMessages(messages: ChatMessageResponse[]) {
  return [...messages].sort((left, right) => {
    const leftTime = left.createdAt ? Date.parse(left.createdAt) : 0;
    const rightTime = right.createdAt ? Date.parse(right.createdAt) : 0;

    return leftTime - rightTime;
  });
}

function getMessageKey(message: ChatMessageResponse) {
  return (
    getMessageId(message) ??
    `${message.senderPublicId ?? "unknown"}:${message.createdAt ?? ""}:${message.content ?? ""}`
  );
}

function mergeMessages(
  currentMessages: ChatMessageResponse[] | undefined,
  nextMessages: ChatMessageResponse[],
) {
  const messagesByKey = new Map<string, ChatMessageResponse>();

  for (const message of [...(currentMessages ?? []), ...nextMessages]) {
    const normalizedMessage = normalizeChatMessage(message);

    if (normalizedMessage) {
      messagesByKey.set(getMessageKey(normalizedMessage), normalizedMessage);
    }
  }

  return sortMessages([...messagesByKey.values()]);
}

function hasMessageContent(
  message: ChatMessageResponse | undefined,
): message is ChatMessageResponse {
  return typeof message?.content === "string" && message.content.length > 0;
}

function formatMessageTime(createdAt: string | undefined, locale: AppLocale) {
  if (!createdAt) {
    return "";
  }

  const date = new Date(createdAt);

  if (Number.isNaN(date.getTime())) {
    return "";
  }

  return new Intl.DateTimeFormat(locale === "de" ? "de-DE" : "en-US", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}

function formatUnreadCount(count: number) {
  return count > 99 ? "99+" : String(count);
}

function ChatDock({
  autoRooms: autoRoomsProp,
  chatPosition,
  currentUserPublicId,
  locale,
  placement = "default",
  t,
}: ChatDockProps) {
  const autoRooms = autoRoomsProp ?? emptyChatRooms;
  const [open, setOpen] = useState(false);
  const [contacts, setContacts] = useState<ChatContact[]>([]);
  const [activeContactId, setActiveContactId] = useState<string>();
  const [draftMessage, setDraftMessage] = useState("");
  const [refreshNonce, setRefreshNonce] = useState(0);
  const contactsRef = useRef<ChatContact[]>([]);
  const friendsByPublicIdRef = useRef<
    Map<number, { avatarUrl?: string; name?: string }>
  >(new Map());
  const messageListRef = useRef<HTMLDivElement>(null);
  const dismissedPrivateActivityRef = useRef<Map<string, string>>(new Map());
  const markedReadByRoomRef = useRef<Map<string, string>>(new Map());
  const previousAutoRoomIdsRef = useRef<Set<string>>(new Set());
  const activeContact = useMemo(
    () => contacts.find((contact) => contact.id === activeContactId),
    [activeContactId, contacts],
  );
  const totalUnreadCount = contacts.reduce(
    (sum, contact) => sum + (contact.unreadCount ?? 0),
    0,
  );

  function updateContact(
    contactId: string,
    updater: (contact: ChatContact) => ChatContact,
  ) {
    setContacts((currentContacts) =>
      currentContacts.map((contact) =>
        contact.id === contactId ? updater(contact) : contact,
      ),
    );
  }

  useEffect(() => {
    contactsRef.current = contacts;
  }, [contacts]);

  useEffect(() => {
    function handleChatRequest(event: Event) {
      if (!isChatRequestEvent(event)) {
        return;
      }

      const friendId = event.detail.friendId;
      const publicId = event.detail.publicId;

      if (!friendId || typeof publicId !== "number") {
        setOpen(true);
        return;
      }

      const nextContact = {
        avatarUrl: event.detail.avatarUrl,
        id: getPrivateContactId(publicId),
        name: event.detail.name ?? t("chat-unknown-contact"),
        targetPublicId: publicId,
        type: "direct" as const,
      };

      dismissedPrivateActivityRef.current.delete(nextContact.id);
      setContacts((currentContacts) => {
        const existingContact = currentContacts.find(
          (contact) => contact.id === nextContact.id,
        );

        if (existingContact) {
          return currentContacts.map((contact) =>
            contact.id === nextContact.id
              ? { ...contact, ...nextContact }
              : contact,
          );
        }

        return [nextContact, ...currentContacts];
      });
      setActiveContactId(nextContact.id);
      setOpen(true);
    }

    window.addEventListener("mira:chat-request", handleChatRequest);

    return () => {
      window.removeEventListener("mira:chat-request", handleChatRequest);
    };
  }, [t]);

  useEffect(() => {
    function handleFriendsUpdated(event: Event) {
      if (!isChatFriendsUpdatedEvent(event)) {
        return;
      }

      const friendsByPublicId = new Map<
        number,
        { avatarUrl?: string; name?: string }
      >();

      for (const friend of event.detail.friends ?? []) {
        if (typeof friend.publicId !== "number") {
          continue;
        }

        friendsByPublicId.set(friend.publicId, {
          avatarUrl: friend.avatarUrl,
          name: friend.name,
        });
      }

      friendsByPublicIdRef.current = friendsByPublicId;
      setContacts((currentContacts) =>
        currentContacts.map((contact) => {
          if (
            contact.type !== "direct" ||
            typeof contact.targetPublicId !== "number"
          ) {
            return contact;
          }

          const friend = friendsByPublicId.get(contact.targetPublicId);

          if (!friend) {
            return contact;
          }

          return {
            ...contact,
            avatarUrl: friend.avatarUrl,
            name: friend.name ?? contact.name,
          };
        }),
      );
    }

    window.addEventListener("mira:friends-updated", handleFriendsUpdated);

    return () => {
      window.removeEventListener("mira:friends-updated", handleFriendsUpdated);
    };
  }, []);

  useEffect(() => {
    const autoRoomIds = new Set(autoRooms.map((room) => room.id));
    const removedGroupRoomIds = contactsRef.current
      .filter((contact) => {
        return (
          contact.locked &&
          isAutoChatRoomId(contact.id) &&
          !autoRoomIds.has(contact.id) &&
          isLastKnownGroupParticipant(contact, currentUserPublicId)
        );
      })
      .map((contact) => contact.roomId)
      .filter((roomId): roomId is string => Boolean(roomId));

    previousAutoRoomIdsRef.current = autoRoomIds;

    for (const roomId of removedGroupRoomIds) {
      void deleteChatRoom({
        baseUrl: CHAT_API_BASE_URL,
        path: { roomId },
      });
    }

    setContacts((currentContacts) => {
      const manualContacts = currentContacts.filter(
        (contact) => !contact.locked && !autoRoomIds.has(contact.id),
      );
      const currentContactsById = new Map(
        currentContacts.map((contact) => [contact.id, contact]),
      );
      const nextContacts = autoRooms.map((room) => {
        const currentContact = currentContactsById.get(room.id);
        const shouldResetRoomState =
          getChatContactResetKey(currentContact) !== getChatContactResetKey(room);

        return {
          ...(shouldResetRoomState ? {} : currentContact),
          ...room,
          locked: true,
        };
      });

      const orderedContacts = [...nextContacts, ...manualContacts];

      return areChatContactsEqual(currentContacts, orderedContacts)
        ? currentContacts
        : orderedContacts;
    });

    setActiveContactId((currentActiveContactId) => {
      if (currentActiveContactId && autoRoomIds.has(currentActiveContactId)) {
        return currentActiveContactId;
      }

      if (currentActiveContactId && !isAutoChatRoomId(currentActiveContactId)) {
        return currentActiveContactId;
      }

      return autoRooms[0]?.id;
    });
  }, [autoRooms]);

  useEffect(() => {
    let cancelled = false;
    const currentPublicId =
      typeof currentUserPublicId === "number" ? currentUserPublicId : undefined;

    async function refreshChatMessages() {
      const roomResult = await listRooms({
        baseUrl: CHAT_API_BASE_URL,
      });

      if (cancelled || roomResult.error) {
        setContacts((currentContacts) =>
          currentContacts.map((contact) =>
            contact.type === "direct" || contact.type === "lobby" || contact.type === "team"
              ? {
                  ...contact,
                  messagesLoading: false,
                }
              : contact,
          ),
        );
        return;
      }

      const rooms = getChatRooms(roomResult.data);
      const contactsById = new Map(
        contactsRef.current.map((contact) => [contact.id, contact]),
      );
      const discoveredContacts: ChatContact[] = [];

      if (typeof currentPublicId === "number") {
        for (const room of rooms) {
          const peerPublicId = getPrivateRoomPeerPublicId(
            room,
            currentPublicId,
          );

          if (
            typeof peerPublicId !== "number" ||
            !hasPrivateRoomActivity(room, currentPublicId)
          ) {
            continue;
          }

          const contactId = getPrivateContactId(peerPublicId);

          if (contactsById.has(contactId)) {
            continue;
          }

          const friendInfo = getFriendContactInfo(
            friendsByPublicIdRef.current,
            peerPublicId,
            `#${peerPublicId}`,
          );
          const roomActivityKey = getRoomActivityKey(room);
          const dismissedActivityKey =
            dismissedPrivateActivityRef.current.get(contactId);

          if (
            dismissedActivityKey &&
            roomActivityKey &&
            dismissedActivityKey === roomActivityKey
          ) {
            continue;
          }

          const discoveredContact = {
            avatarUrl: friendInfo.avatarUrl,
            id: contactId,
            lastActivityKey: roomActivityKey,
            lastReadAt: room.lastReadAt,
            messagesLoaded: false,
            name: friendInfo.name,
            roomId: getRoomId(room),
            targetPublicId: peerPublicId,
            type: "direct" as const,
            unreadCount: getRoomUnreadCount(room, currentPublicId),
          };

          contactsById.set(contactId, discoveredContact);
          discoveredContacts.push(discoveredContact);
        }
      }

      if (discoveredContacts.length > 0) {
        setContacts((currentContacts) => {
          const currentContactIds = new Set(
            currentContacts.map((contact) => contact.id),
          );
          const nextDiscoveredContacts = discoveredContacts.filter(
            (contact) => !currentContactIds.has(contact.id),
          );

          return nextDiscoveredContacts.length > 0
            ? [...nextDiscoveredContacts, ...currentContacts]
            : currentContacts;
        });
      }

      const messageContacts = [...contactsById.values()].filter(
        (contact) =>
          contact.type === "direct" ||
          contact.type === "lobby" ||
          contact.type === "team",
      );

      if (messageContacts.length === 0) {
        return;
      }

      setContacts((currentContacts) =>
        currentContacts.map((contact) =>
          contact.type === "direct" || contact.type === "lobby" || contact.type === "team"
            ? {
                ...contact,
                messagesLoading: !contact.messagesLoaded,
              }
            : contact,
        ),
      );

      for (const contact of messageContacts) {
        if (cancelled) {
          return;
        }

        const room = findChatRoomForContact(
          rooms,
          contact,
          currentPublicId,
        );

        const roomId = getRoomId(room);

        if (!room || !roomId) {
          updateContact(contact.id, (currentContact) => ({
            ...currentContact,
            messages: currentContact.messages ?? [],
            messagesLoaded: true,
            messagesLoading: false,
          }));
          continue;
        }

        const messagesResult = await listMessages({
          baseUrl: CHAT_API_BASE_URL,
          path: {
            roomId,
          },
          query: {
            limit: chatMessageLimit,
          },
        });

        if (cancelled) {
          return;
        }

        updateContact(contact.id, (currentContact) => {
          const nextMessages = messagesResult.error
            ? room.lastMessage
              ? normalizeChatMessages([room.lastMessage])
              : []
            : getChatMessages(messagesResult.data);

          return {
            ...currentContact,
            lastActivityKey: getRoomActivityKey(room) ?? currentContact.lastActivityKey,
            lastReadAt: room.lastReadAt ?? currentContact.lastReadAt,
            messages: mergeMessages(currentContact.messages, nextMessages),
            messagesLoaded: true,
            messagesLoading: false,
            roomId,
            unreadCount:
              typeof currentPublicId === "number"
                ? getRoomUnreadCount(room, currentPublicId)
                : (room.unreadCount ?? currentContact.unreadCount ?? 0),
          };
        });
      }
    }

    void refreshChatMessages();

    const refreshInterval = window.setInterval(
      () => void refreshChatMessages(),
      chatRefreshMs,
    );

    return () => {
      cancelled = true;
      window.clearInterval(refreshInterval);
    };
  }, [activeContactId, currentUserPublicId, open, refreshNonce]);

  useEffect(() => {
    if (!open || !activeContactId || !activeContact?.roomId) {
      return;
    }

    const latestMessage =
      activeContact.messages?.[(activeContact.messages?.length ?? 0) - 1];
    const latestMessageId = getMessageId(latestMessage);
    const readMarker = latestMessageId ?? activeContact.lastReadAt ?? "";
    const activeRoomId = activeContact.roomId;

    if (!activeRoomId) {
      return;
    }

    const previousReadMarker = markedReadByRoomRef.current.get(activeRoomId);

    if (!activeContact.unreadCount || previousReadMarker === readMarker) {
      return;
    }

    markedReadByRoomRef.current.set(activeRoomId, readMarker);
    updateContact(activeContactId, (contact) => ({
      ...contact,
      unreadCount: 0,
    }));

    void markRead({
      baseUrl: CHAT_API_BASE_URL,
      body: latestMessageId
        ? {
            messageId: latestMessageId,
          }
        : undefined,
      path: {
        roomId: activeRoomId,
      },
    }).then((result) => {
      if (result.error || !result.data) {
        markedReadByRoomRef.current.delete(activeRoomId);
        return;
      }

      updateContact(activeContactId, (contact) => ({
        ...contact,
        lastReadAt: result.data?.lastReadAt ?? contact.lastReadAt,
        unreadCount: result.data?.unreadCount ?? 0,
      }));
    });
  }, [
    activeContact?.lastReadAt,
    activeContact?.messages,
    activeContact?.roomId,
    activeContact?.unreadCount,
    activeContactId,
    open,
  ]);

  useEffect(() => {
    const messageList = messageListRef.current;

    if (!messageList) {
      return;
    }

    messageList.scrollTop = messageList.scrollHeight;
  }, [activeContact?.messages?.length, activeContactId]);

  async function submitDraftMessage() {
    const message = draftMessage.trim();

    if (
      !message ||
      !activeContact ||
      !canSendToContact(activeContact)
    ) {
      return;
    }

    setDraftMessage("");
    updateContact(activeContact.id, (contact) => ({
      ...contact,
      sendError: false,
      sending: true,
    }));

    const body = {
      content: message,
    };
    let result: ChatSendResult | undefined;

    if (activeContact.type === "direct") {
      result = await sendPrivate({
            baseUrl: CHAT_API_BASE_URL,
            body,
            path: {
              targetPublicId: activeContact.targetPublicId as number,
            },
          });
    } else if (activeContact.type === "lobby") {
      result = await sendLobbyMessage({
              baseUrl: CHAT_API_BASE_URL,
              body,
              path: {
                lobbyId: activeContact.contextId as string,
              },
            });
    } else {
      const teamPathCandidates = getChatTeamPathCandidates(activeContact.team);

      for (const teamPath of teamPathCandidates) {
        result = await sendTeamMessage({
          baseUrl: CHAT_API_BASE_URL,
          body,
          path: {
            matchId: activeContact.contextId as string,
            team: teamPath,
          },
        });

        if (!result.error) {
          break;
        }
      }
    }

    if (!result || result.error) {
      setDraftMessage((currentDraftMessage) => currentDraftMessage || message);
      updateContact(activeContact.id, (contact) => ({
        ...contact,
        sendError: true,
        sending: false,
      }));
      return;
    }

    const sentMessage: ChatMessageResponse = hasMessageContent(result.data)
      ? (normalizeChatMessage(result.data) ?? result.data)
      : {
          content: message,
          createdAt: new Date().toISOString(),
          roomId: activeContact.roomId,
          senderPublicId: currentUserPublicId,
        };

    updateContact(activeContact.id, (contact) => ({
      ...contact,
      messages: mergeMessages(contact.messages, [sentMessage]),
      messagesLoaded: hasMessageContent(result.data) ? true : contact.messagesLoaded,
      roomId: getMessageRoomId(sentMessage) ?? contact.roomId,
      sendError: false,
      sending: false,
    }));
    setRefreshNonce((currentRefreshNonce) => currentRefreshNonce + 1);
  }

  function closeContact(contactId: string) {
    const contactToClose = contacts.find((contact) => contact.id === contactId);

    if (contactToClose?.locked) {
      return;
    }

    if (contactToClose?.type === "direct") {
      const latestMessage =
        contactToClose.messages?.[(contactToClose.messages?.length ?? 0) - 1];
      const activityKey =
        getMessageActivityKey(latestMessage) ??
        contactToClose.lastActivityKey ??
        contactToClose.lastReadAt ??
        contactToClose.roomId ??
        contactToClose.id;

      dismissedPrivateActivityRef.current.set(contactToClose.id, activityKey);
    }

    const remainingContacts = contacts.filter((contact) => contact.id !== contactId);

    setContacts(remainingContacts);

    if (activeContactId !== contactId) {
      return;
    }

    setActiveContactId(remainingContacts[0]?.id);
  }

  const toggleButton = (
    <button
      aria-expanded={open}
      aria-label={t(open ? "chat-close" : "chat-open")}
      className={
        [
          "chat-dock-tab",
          chatPosition === "left" ? "chat-dock-tab-left" : "",
          totalUnreadCount > 0 && !open ? "chat-dock-tab-attention" : "",
        ].filter(Boolean).join(" ")
      }
      data-placement={placement}
      type="button"
      onClick={() => setOpen((currentOpen) => !currentOpen)}
    >
      <MessageCircle size={19} />
      {totalUnreadCount > 0 ? (
        <span aria-hidden="true" className="chat-unread-badge">
          {formatUnreadCount(totalUnreadCount)}
        </span>
      ) : null}
    </button>
  );
  const activeMessages = activeContact?.messages ?? [];
  const composerDisabled =
    !activeContact || !canSendToContact(activeContact);

  return (
    <>
      {chatPosition === "left" ? toggleButton : null}
      <section
        aria-label={t("chat-title")}
        className={open ? "chat-dock open" : "chat-dock"}
        data-placement={placement}
        data-position={chatPosition}
      >
        {chatPosition === "right" ? toggleButton : null}

        <div className="chat-dock-window">
          <header className="chat-dock-header">
            <div className="chat-dock-title">
              <MessageCircle size={17} />
              <span>{t("chat-title")}</span>
            </div>
            <span>{activeContact?.name ?? t("chat-no-active")}</span>
          </header>

          <div className="chat-dock-body">
            <aside className="chat-contact-list" aria-label={t("chat-contacts")}>
              {contacts.length > 0 ? (
                contacts.map((contact) => (
                  <div
                    aria-selected={activeContactId === contact.id}
                    className="chat-contact-card"
                    key={contact.id}
                    role="option"
                  >
                    <button
                      className="chat-contact-button"
                      type="button"
                      onClick={() => setActiveContactId(contact.id)}
                    >
                      <span className="chat-contact-avatar" aria-hidden="true">
                        {getProfileInitials(contact.name)}
                        {contact.avatarUrl ? (
                          <img alt="" src={contact.avatarUrl} />
                        ) : null}
                      </span>
                      <span className="chat-contact-name">{contact.name}</span>
                      {(contact.unreadCount ?? 0) > 0 ? (
                        <span
                          aria-hidden="true"
                          className="chat-contact-unread-badge"
                        >
                          {formatUnreadCount(contact.unreadCount ?? 0)}
                        </span>
                      ) : null}
                    </button>
                    {contact.locked ? (
                      <span
                        aria-hidden="true"
                        className="chat-contact-room-marker"
                        title={contact.subtitle}
                      >
                        {contact.type === "team" ? "5" : "L"}
                      </span>
                    ) : (
                      <button
                        className="chat-contact-close"
                        type="button"
                        aria-label={t("chat-close-card")}
                        onClick={() => closeContact(contact.id)}
                      >
                        <X size={13} />
                      </button>
                    )}
                  </div>
                ))
              ) : (
                <p className="chat-empty-state">{t("chat-empty")}</p>
              )}
            </aside>

            <div className="chat-thread">
              <div className="chat-message-list" ref={messageListRef} role="log">
                {activeContact && activeMessages.length > 0 ? (
                  activeMessages.map((message) => {
                    const ownMessage =
                      typeof currentUserPublicId === "number" &&
                      message.senderPublicId === currentUserPublicId;
                    const messageTime = formatMessageTime(
                      message.createdAt,
                      locale,
                    );
                    const senderInfo = getMessageSenderInfo(
                      activeContact,
                      message.senderPublicId,
                      friendsByPublicIdRef.current,
                      t,
                    );
                    const showSenderInfo = activeContact.type !== "direct";

                    return (
                      <article
                        className="chat-message-item"
                        data-own={ownMessage ? "true" : "false"}
                        key={
                          getMessageId(message) ??
                          `${message.senderPublicId ?? "unknown"}:${message.createdAt ?? ""}:${message.content ?? ""}`
                        }
                      >
                        {showSenderInfo ? (
                          <div className="chat-message-sender">
                            <span className="chat-message-avatar" aria-hidden="true">
                              {getProfileInitials(senderInfo.name)}
                              {senderInfo.avatarUrl ? (
                                <img alt="" src={senderInfo.avatarUrl} />
                              ) : null}
                            </span>
                            <span>{senderInfo.name}</span>
                          </div>
                        ) : null}
                        <p>{message.content}</p>
                        {messageTime ? <time>{messageTime}</time> : null}
                      </article>
                    );
                  })
                ) : activeContact ? (
                  <p className="chat-empty-state">{t("chat-thread-empty")}</p>
                ) : (
                  <p className="chat-empty-state">{t("chat-no-active-body")}</p>
                )}
              </div>

              <form
                className="chat-composer"
                onSubmit={(event) => {
                  event.preventDefault();
                  submitDraftMessage();
                }}
              >
                <input
                  aria-label={t("chat-message")}
                  disabled={composerDisabled}
                  placeholder={t("chat-message")}
                  value={draftMessage}
                  onChange={(event) => {
                    setDraftMessage(event.target.value);
                    if (activeContact?.sendError) {
                      updateContact(activeContact.id, (contact) => ({
                        ...contact,
                        sendError: false,
                      }));
                    }
                  }}
                />
                <button
                  aria-label={t("chat-send")}
                  disabled={composerDisabled || !draftMessage.trim()}
                  type="submit"
                >
                  <Send size={16} />
                </button>
                {activeContact?.sendError ? (
                  <p className="chat-composer-error" role="status">
                    {t("chat-send-failed")}
                  </p>
                ) : null}
              </form>
            </div>
          </div>
        </div>
      </section>
    </>
  );
}

export default ChatDock;
