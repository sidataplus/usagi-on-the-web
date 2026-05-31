class ProjectMembership < ProjectMember
  self.table_name = "project_members"
  prefixed_id "pmem"
end
