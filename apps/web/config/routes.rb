Rails.application.routes.draw do
  get "up" => "rails/health#show", as: :rails_health_check

  root "projects#index"

  resource :session, only: %i[new create destroy]

  get "privacy", to: "pages#privacy"
  get "terms", to: "pages#terms"

  resources :projects do
    member do
      get :manage
    end

    resources :import_sessions, only: %i[index new create show] do
      collection do
        post :preview
      end

      member do
        post :confirm
      end
    end
    resources :imports, only: %i[index new create show], controller: "import_sessions"
    resources :engine_jobs, only: %i[index show] do
      member do
        post :retry
      end
    end
    resources :mappings, only: %i[index]
    resources :members, only: %i[create update destroy], controller: "project_members"
    resources :exports, only: %i[index create show]
    resource :export, only: %i[show], controller: "exports"
    resource :auto_map, only: %i[create]
  end

  resources :mappings, only: %i[show update] do
    resources :comments, only: %i[index create]
    resources :mapping_candidates, only: %i[index]
    resource :manual_search, only: %i[create]

    member do
      post :apply_candidate
      post :approve
      post :flag
      post :invalidate
      patch :status
      get :history
      get :candidates
      get :concept_search
    end
  end

  resources :mapping_candidates, only: %i[update]
  post "/mappings/bulk_update", to: "mappings#bulk_update", as: :bulk_update_mappings

  get "dashboard", to: "dashboard#show"
  get "settings", to: "settings#show"
  patch "settings", to: "settings#update"

  namespace :admin do
    root to: "engine_status#show"
    resource :engine_status, only: %i[show], controller: "engine_status"
    resources :engine_builds, only: %i[create]
    resources :users, only: %i[index update]
    resources :audit_logs, only: %i[index]
  end

  match "*path", to: "pages#not_found", via: :all,
        constraints: ->(req) { req.path.exclude?("rails/") && req.path.exclude?(".") }
end
