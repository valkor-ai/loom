function validationError(code, message) {
  const error = new Error(message);
  error.code = code;
  error.status = 400;
  return error;
}

function cloneMember(member) {
  return { ...member };
}

function cloneInvite(invite) {
  return { ...invite };
}

function createWorkspaceRegistry({ now = () => new Date() } = {}) {
  const members = new Map();
  const invites = new Map();
  let nextInviteId = 1;

  function timestamp() {
    return now().toISOString();
  }

  function addMember(input) {
    const id = String(input?.id || "").trim();
    const email = String(input?.email || "").trim();
    if (!id) {
      throw validationError("INVALID_MEMBER_ID", "Member id is required.");
    }
    if (!email) {
      throw validationError("INVALID_EMAIL", "Member email is required.");
    }
    if (members.has(id)) {
      throw validationError("DUPLICATE_MEMBER", "Member already exists.");
    }

    const createdAt = timestamp();
    const member = {
      id,
      email,
      role: input?.role ? String(input.role) : "viewer",
      createdAt,
      updatedAt: createdAt
    };
    members.set(id, member);
    return cloneMember(member);
  }

  function setRole(id, role) {
    const member = members.get(id);
    if (!member) {
      throw validationError("MEMBER_NOT_FOUND", "Member was not found.");
    }
    member.role = String(role || "viewer");
    member.updatedAt = timestamp();
    return cloneMember(member);
  }

  function removeMember(id) {
    return members.delete(id);
  }

  function inviteMember(input) {
    const email = String(input?.email || "").trim();
    if (!email) {
      throw validationError("INVALID_EMAIL", "Invite email is required.");
    }

    const createdAt = timestamp();
    const invite = {
      id: String(nextInviteId++),
      email,
      role: input?.role ? String(input.role) : "viewer",
      status: "pending",
      createdAt,
      updatedAt: createdAt
    };
    invites.set(invite.id, invite);
    return cloneInvite(invite);
  }

  function listMembers(filter = {}) {
    let result = [...members.values()];
    if (filter.role) {
      result = result.filter((member) => member.role === filter.role);
    }
    return result.map(cloneMember);
  }

  function listInvites(filter = {}) {
    let result = [...invites.values()];
    if (filter.status) {
      result = result.filter((invite) => invite.status === filter.status);
    }
    return result.map(cloneInvite);
  }

  function summarizeWorkspace() {
    return {
      members: members.size,
      invites: invites.size,
      generatedAt: timestamp()
    };
  }

  return {
    addMember,
    setRole,
    removeMember,
    inviteMember,
    listMembers,
    listInvites,
    summarizeWorkspace
  };
}

module.exports = {
  createWorkspaceRegistry,
  validationError
};
