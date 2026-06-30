import { MessageCircle, Send, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { Translate } from "../types/ui";
import { getProfileInitials } from "../utils/profile";
import type { ChatPosition } from "../settings";

type ChatDockProps = {
  chatPosition: ChatPosition;
  t: Translate;
};

type ChatContact = {
  avatarUrl?: string;
  id: string;
  name: string;
};

type ChatRequestEvent = CustomEvent<{
  avatarUrl?: string;
  friendId?: string;
  name?: string;
}>;

function isChatRequestEvent(event: Event): event is ChatRequestEvent {
  return event.type === "mira:chat-request";
}

function ChatDock({ chatPosition, t }: ChatDockProps) {
  const [open, setOpen] = useState(false);
  const [contacts, setContacts] = useState<ChatContact[]>([]);
  const [activeContactId, setActiveContactId] = useState<string>();
  const [draftMessage, setDraftMessage] = useState("");
  const activeContact = useMemo(
    () => contacts.find((contact) => contact.id === activeContactId),
    [activeContactId, contacts],
  );

  useEffect(() => {
    function handleChatRequest(event: Event) {
      if (!isChatRequestEvent(event)) {
        return;
      }

      const friendId = event.detail.friendId;

      if (!friendId) {
        setOpen(true);
        return;
      }

      const nextContact = {
        avatarUrl: event.detail.avatarUrl,
        id: friendId,
        name: event.detail.name ?? t("chat-unknown-contact"),
      };

      setContacts((currentContacts) => {
        const existingContact = currentContacts.find(
          (contact) => contact.id === friendId,
        );

        if (existingContact) {
          return currentContacts.map((contact) =>
            contact.id === friendId ? { ...contact, ...nextContact } : contact,
          );
        }

        return [nextContact, ...currentContacts];
      });
      setActiveContactId(friendId);
      setOpen(true);
    }

    window.addEventListener("mira:chat-request", handleChatRequest);

    return () => {
      window.removeEventListener("mira:chat-request", handleChatRequest);
    };
  }, [t]);

  function submitDraftMessage() {
    const message = draftMessage.trim();

    if (!message || !activeContact) {
      return;
    }

    setDraftMessage("");
  }

  function closeContact(contactId: string) {
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
        chatPosition === "left"
          ? "chat-dock-tab chat-dock-tab-left"
          : "chat-dock-tab"
      }
      type="button"
      onClick={() => setOpen((currentOpen) => !currentOpen)}
    >
      <MessageCircle size={19} />
    </button>
  );

  return (
    <>
      {chatPosition === "left" ? toggleButton : null}
      <section
        aria-label={t("chat-title")}
        className={open ? "chat-dock open" : "chat-dock"}
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
                      <span>{contact.name}</span>
                    </button>
                    <button
                      className="chat-contact-close"
                      type="button"
                      aria-label={t("chat-close-card")}
                      onClick={() => closeContact(contact.id)}
                    >
                      <X size={13} />
                    </button>
                  </div>
                ))
              ) : (
                <p className="chat-empty-state">{t("chat-empty")}</p>
              )}
            </aside>

            <div className="chat-thread">
              <div className="chat-message-list">
                {activeContact ? (
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
                  disabled={!activeContact}
                  placeholder={t("chat-message")}
                  value={draftMessage}
                  onChange={(event) => setDraftMessage(event.target.value)}
                />
                <button
                  aria-label={t("chat-send")}
                  disabled={!activeContact || !draftMessage.trim()}
                  type="submit"
                >
                  <Send size={16} />
                </button>
              </form>
            </div>
          </div>
        </div>
      </section>
    </>
  );
}

export default ChatDock;
